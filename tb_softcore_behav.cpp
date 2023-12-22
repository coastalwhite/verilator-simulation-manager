#include <cstdlib>
#include <cstring>
#include <iostream>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <verilated.h>
#include <verilated_vcd_c.h>
#include "VSECURE_PLATFORM_RI5CY_CW.h"
#include <time.h>

#include "svdpi.h"
#include "VSECURE_PLATFORM_RI5CY_CW__Dpi.h"
#include <ForkClient.hpp>
#include <Protocol.hpp>
#include <coverage.hpp>

#define MAX_SIM_TIME 1000
vluint64_t sim_time = 0;

void run_fork(VSECURE_PLATFORM_RI5CY_CW* dut, Socket *socket) {
    Message msg = Message::read_from_socket(socket->socket_fd);

	uint8_t idata[4*8] = {
        0x13, 0x00, 0x00, 0x00,
        0x13, 0x00, 0x00, 0x00,
        0x13, 0x00, 0x00, 0x00,
        0x13, 0x00, 0x00, 0x00,
        0x13, 0x00, 0x00, 0x00,
        0x13, 0x00, 0x00, 0x00,
        0x13, 0x00, 0x00, 0x00,
        0x6F, 0xF0, 0x5F, 0xFF,
	};

    if ( msg.variant == MSG_DATA ) {
        uint8_t* ptr = msg.content.bytearray.ptr;
        uint32_t len = msg.content.bytearray.len;
		
		for (int i = 0; i < len; i++) {
			idata[i] = ptr[i];
		}
    }

	int start = 0x80;
	int length = 4;

    dut->crypto_start = 0;

    while (sim_time < MAX_SIM_TIME) {
        dut->clock ^= 1;
		dut->reset = (sim_time < 3) ? 0 : 1;

        if (sim_time == 4) {
            const svScope scope = svGetScopeFromName("TOP.SECURE_PLATFORM_RI5CY_CW.uAHB2MEM.ram");
            assert(scope);  // Check for nullptr if scope not found
            svSetScope(scope);
            
            for (int i = 0; i < 4*8; i++) {
                setInstructionMemory(start + i, idata[i]);
            }
        }
        
        dut->eval();

        sim_time++;
    }

    uint8_t* data = (uint8_t*) malloc(COVMAP_SIZE / 2);
    memcpy(data, __vri_covmap.Compress(), COVMAP_SIZE / 2);

    Message::data_msg(data, COVMAP_SIZE / 2).write_to_socket(socket->socket_fd);

    // This just instantly kills the process and does not try to even cleanup
    // anything.
    socket->await_exit();

    return;
}

int main(const int argc, const char **argv, const char **env) {
    ForkClient client(argc, argv);

    client.send_input_width(32);

	signal(SIGCHLD, SIG_IGN);

    VSECURE_PLATFORM_RI5CY_CW *dut = new VSECURE_PLATFORM_RI5CY_CW;

    // run_fork(dut, nullptr);
	//

	int i = 0;
	while (1) {
		i++;

		Socket *result = client.await_fork();

		if (result != nullptr) {
            run_fork(dut, result);

			delete result;
			delete dut;
            exit(EXIT_SUCCESS);
		}
	}

	delete dut;
    exit(EXIT_SUCCESS);
}
