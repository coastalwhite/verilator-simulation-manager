use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use fxhash::FxHashSet;
use rand::{SeedableRng, Rng};
use rand_chacha::ChaCha20Rng;
use vl_fork_server::{ForkServer, ProtocolResult, Message, ProtocolError};

use crate::mutator::{BitFlip, FuzzDimensions, NibleFlip, ByteFlip};

use self::monitor::Monitor;

pub mod monitor;
pub mod mutator;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuzzInput {
    content: Vec<u8>,
}

pub struct FuzzServer {
    fork_server: ForkServer,

    exit_request_channel: Receiver<()>,

    current_datas: Vec<Option<FuzzInput>>,

    mutator: FuzzMutator,

    run_queue: Vec<FuzzInput>,
    mutation_queue: Vec<FuzzInput>,
}

pub struct FuzzFeedback {
    hash: u64,
    data: Vec<u8>,
}

pub struct FuzzMutator {
    width: u32,
    seen: FxHashSet<FuzzInput>,
    rng: ChaCha20Rng,
}

impl FuzzMutator {
    pub fn new(width: u32, rng: ChaCha20Rng) -> Self {
        Self {
            width,
            seen: FxHashSet::default(),
            rng,
        }
    }

    pub fn deterministic_mutate(&mut self, queue: &mut Vec<FuzzInput>, input: &[u8]) {
        use mutator::Mutator;

        let dims = FuzzDimensions {
            bit_width: self.width,
            num_cycles: 1,
        };

        let mut bitflip = BitFlip;
        let mut nible_flip = NibleFlip;
        let mut byte_flip = ByteFlip;

        for i in 0..bitflip.num_possible_mutations(dims) {
            let input = FuzzInput { content: bitflip.apply(dims, i, input) };

            if self.seen.insert(input.clone()) {
                queue.push(input);
            }
        }

        for i in 0..nible_flip.num_possible_mutations(dims) {
            let input = FuzzInput { content: nible_flip.apply(dims, i, input) };

            if self.seen.insert(input.clone()) {
                queue.push(input);
            }
        }

        for i in 0..byte_flip.num_possible_mutations(dims) {
            let input = FuzzInput { content: byte_flip.apply(dims, i, input) };

            if self.seen.insert(input.clone()) {
                queue.push(input);
            }
        }
    }
}
 
impl FuzzServer {
    pub fn new(cmd: &Path, exit_request_channel: Receiver<()>) -> ProtocolResult<Self> {
        let mut fork_server = ForkServer::new(
            &cmd,
            &Path::new("/tmp"),
            Some(Duration::from_secs(1)),
            Some(Duration::from_secs(1)),
        )?;

        let Message::InputWidth { width } = fork_server.server_socket().read_message()? else {
            return Err(ProtocolError::other(
                "Expected input width message from fork server",
            ));
        };

        let rng = ChaCha20Rng::from_seed([0u8; 32]);

        Ok(Self {
            fork_server,
            exit_request_channel,
            current_datas: Vec::new(),
            mutator: FuzzMutator::new(width, rng),
            run_queue: Vec::new(),
            mutation_queue: Vec::new(),
        })
    }

    fn on_exec(&mut self, idx: usize) -> ProtocolResult<()> {
        if self.run_queue.is_empty() {
            self.refresh_queue();
        }

        let data = self.run_queue.pop().unwrap();
        self.current_datas[idx] = Some(data.clone());
        self.fork_server
            .get_fork(idx)
            .unwrap()
            .send_message(&Message::Data {
                content: data.content,
            })?;

        Ok(())
    }

    fn try_finish(&mut self, at: usize) -> ProtocolResult<Option<FuzzFeedback>> {
        Ok(self.fork_server.try_finish(at)?.map(|data| {
            let hash = fxhash::hash64(&data);
            FuzzFeedback { hash, data }
        }))
    }

    pub fn refresh_queue(&mut self) {
        println!("Refreshing Queue");

        if self.mutation_queue.is_empty() {
            println!("No items in mutation queue, starting from random.");

            let size = self.mutator.width.div_ceil(8) as usize;
            let mut content = Vec::with_capacity(size);

            for _ in 0..size {
                content.push(self.mutator.rng.gen());
            }

            self.run_queue.push(FuzzInput { content });
        } else {
            for candidate in self.mutation_queue.iter() {
                self.mutator
                    .deterministic_mutate(&mut self.run_queue, &candidate.content);
            }
            self.mutation_queue.clear();
        }

        println!("New queue has {} inputs.", self.run_queue.len());
    }

    pub fn fuzz_loop<M: Monitor>(
        &mut self,
        num_children: u16,
        iterations: Option<u64>,
    ) -> ProtocolResult<()> {
        let mut num_iters = 0;

        for _ in 0..num_children {
            self.current_datas.push(None);

            num_iters += 1;

            let idx = self.fork_server.create_fork()?;
            self.on_exec(idx)?;
        }

        let mut fork_offset = 0;
        let mut branches_found = 0u64;

        let mut seen_covmaps = FxHashSet::default();

        let mut monitor = M::init();
        loop {
            fork_offset += 1;
            fork_offset %= num_children;

            if self.exit_request_channel.try_recv().is_ok() {
                println!("[FUZZ SERVER]: Received stop signal");
                return Ok(());
            }

            monitor.on_loop(num_iters);

            if let Some(FuzzFeedback { hash, data: _ }) = self.try_finish(fork_offset as usize)? {
                let is_new = seen_covmaps.insert(hash);

                if is_new {
                    branches_found += 1;

                    if branches_found % 100 == 0 {
                        println!("Found {branches_found} branches");
                    }

                    self.mutation_queue.push(
                        self.current_datas[fork_offset as usize]
                            .as_ref()
                            .unwrap()
                            .clone(),
                    );
                }

                num_iters += 1;
                if iterations.is_some_and(|iterations| iterations <= num_iters) {
                    break;
                }

                if self.run_queue.is_empty() {
                    self.refresh_queue();
                }

                self.fork_server.replace_fork(fork_offset as usize).unwrap();
                self.on_exec(fork_offset as usize)?;
            }
        }

        println!("[FUZZ SERVER]: Done with iterations");

        Ok(())
    }

    pub fn clean(mut self) -> ProtocolResult<()> {
        self.fork_server.clean()
    }
}
