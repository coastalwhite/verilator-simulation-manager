#include "ForkClient.hpp"

#include "Protocol.hpp"
#include "ProtocolUtil.hpp"
#include "SharedMemory.hpp"

#include <cstdlib>
#include <filesystem>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>

Socket::Socket() : socket_fd(0) {}
Socket::Socket(const char *socket_path) {
    int socket_fd = socket(AF_UNIX, SOCK_STREAM, 0);

    struct sockaddr_un address;

    address.sun_family = AF_UNIX;

    strcpy(address.sun_path, socket_path);

    if (connect(socket_fd, (struct sockaddr *)&address, sizeof(address)) < 0) {
        perror("Failed to connect to socket");
        exit(EXIT_FAILURE);
    }

    this->socket_fd = socket_fd;
}

ForkClient::ForkClient(const int argc, const char **argv) {
    if (argc < 3) {
        fprintf(stderr, "ForkClient two args. Socket path and SHM id.");
        exit(2);
    }

    const char *socket_path = argv[1];
    const char *activations_path = argv[2];

    ;

    this->socket = new Socket(socket_path);
    this->activations = new SharedMemory(activations_path, ACCESS_READWRITE,
                                         MAX_FORKS * 2 * sizeof(uint32_t));

    this->num_forks = 0;
    this->fork_shm_ids = nullptr;
}

ForkClient::~ForkClient() {
    if (this->fork_shm_ids != nullptr) {
        for (uint32_t i = 0; i < this->num_forks; i++) {
            free(this->fork_shm_ids[i]);
        }
        free(this->fork_shm_ids);
    }

    delete this->socket;
    delete this->activations;
}

void ForkClient::send_input_info(uint32_t width) {
    Message::input_width(width).write_to_socket(this->socket->socket_fd);
}

void Socket::clean_exit() {
    if (shutdown(this->socket_fd, SHUT_WR) != 0) {
        perror("Failed to shutdown socket");
        exit(1);
    }

    raise(SIGQUIT);
}

void ForkClient::prepare_forks() {
    Message msg =
        Message::expect_from_socket(this->socket->socket_fd, MSG_NUM_FORKS);

    this->num_forks = msg.content.integer;
    this->fork_shm_ids = (char **)malloc(num_forks * sizeof(char *));

    if (this->fork_shm_ids == nullptr) {
        perror("Failed to allocate memory for fork_shm_ids");
        exit(1);
    }

    for (uint32_t i = 0; i < num_forks; i++) {
        Message msg =
            Message::expect_from_socket(this->socket->socket_fd, MSG_FORK);

        uint32_t input_len = msg.content.fork_info.input.len;
        uint32_t output_len = msg.content.fork_info.output.len;

        char *input = (char *)malloc(input_len + 1);
        char *output = (char *)malloc(output_len + 1);

        if (input == nullptr) {
            perror("Failed to allocate memory for input");
            exit(1);
        }
        if (output == nullptr) {
            perror("Failed to allocate memory for output");
            exit(1);
        }

        memcpy(input, msg.content.fork_info.input.ptr, input_len);
        memcpy(output, msg.content.fork_info.output.ptr, output_len);

        input[input_len] = 0;
        output[output_len] = 0;

        this->fork_shm_ids[i * 2 + 0] = input;
        this->fork_shm_ids[i * 2 + 1] = output;
    }
}

fork_result_t ForkClient::block_till_fork_or_exit() {
    uint32_t *activations = (uint32_t *)this->activations->get_data();

    while (1) {
        for (uint32_t i = 0; i < this->num_forks; i++) {
            if (activations[i * 2] != ACTIVATION_STARTABLE) {
                continue;
            }

            activations[i * 2] = ACTIVATION_ACTIVE;

            fork_result_t result;

            result.input_size = activations[i * 2 + 1];
            result.input = this->fork_shm_ids[i * 2];
            result.output = this->fork_shm_ids[i * 2 + 1];

            return result;
        }

        quit_if_no_parent();
        sched_yield();
    }
}

SharedMemory ForkClient::input_shm(fork_result_t fork) {
    return SharedMemory(fork.input, ACCESS_READONLY, SHM_MAX_INPUT_SIZE);
}

SharedMemory ForkClient::output_shm(fork_result_t fork) {
    return SharedMemory(fork.output, ACCESS_READWRITE, SHM_OUTPUT_SIZE);
}