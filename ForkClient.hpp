#ifndef __FORK_CLIENT_H__
#define __FORK_CLIENT_H__

#include "SharedMemory.hpp"
#include <stdint.h>

#define SHM_MAX_INPUT_SIZE (1 << 10)
#define SHM_OUTPUT_SIZE (1 << 16)
#define MAX_FORKS 512

class Socket {
  public:
    int socket_fd;

    Socket();
    Socket(const char *socket_path);
    
    void clean_exit();
};

typedef enum {
	ACTIVATION_INACTIVE  = 0,
	ACTIVATION_DONE      = 1,
	ACTIVATION_STARTABLE = 2,
	ACTIVATION_ACTIVE    = 3,
} activation_t;

struct fork_result_t {
	uint32_t input_size;
	const char* input;
	const char* output;
};

class ForkClient {
  private:
    Socket *socket;

	/// This is a shared memory error with all the processes. It consists of
	/// a one dimensional u32 array with `num_forks * 2` elements.
	/// For each fork:
	/// - Activation Status
	/// - Input Size
	SharedMemory *activations;

	/// The # of forks
	uint32_t num_forks;
	/// The IDs for the SharedMemories of the forks
	char **fork_shm_ids;

  public:
    ForkClient(const int argc, const char **argv);
    ~ForkClient();

    void send_input_info(uint32_t width);
	void prepare_forks();
    fork_result_t block_till_fork_or_exit();

    static SharedMemory input_shm(fork_result_t fork);
    static SharedMemory output_shm(fork_result_t fork);
};

#endif // __FORK_CLIENT_H__