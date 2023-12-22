#include "ForkClient.hpp"

#include "Protocol.hpp"
#include <filesystem>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <signal.h>
#include <sys/un.h>
#include <unistd.h>

Socket::Socket() : socket_fd(0) {}

Socket::Socket(const char *socket_path) {
    int socket_fd = socket(AF_UNIX, SOCK_STREAM, 0);

    struct sockaddr_un address;

    address.sun_family = AF_UNIX;

    strcpy(address.sun_path, socket_path);

    if (connect(socket_fd, (struct sockaddr *)&address, sizeof(address)) < 0) {
        perror("Connecting to socket failed");
        exit(EXIT_FAILURE);
    }

    this->socket_fd = socket_fd;
}

ForkClient::ForkClient(const int argc, const char **argv) {
    if (argc < 2) {
        perror("[ERROR]: ForkClient expects the first argument to be the "
               "socket path");
        exit(2);
    }

    const char *socket_path = argv[1];

    this->socket = new Socket(socket_path);
}

ForkClient::~ForkClient() { delete this->socket; }

void ForkClient::send_input_width(uint32_t width) {
    Message::input_width(width).write_to_socket(this->socket->socket_fd);
}

void Socket::await_exit() {
	shutdown(this->socket_fd, SHUT_WR);

    Message msg = Message::read_from_socket(this->socket_fd);

    if (msg.variant != MSG_EXIT) {
        perror("Expected exit message got something else");
        exit(1);
    }

    close(this->socket_fd);
    raise(SIGQUIT);
}

Socket *ForkClient::await_fork() {
    while (1) {
        Message msg = Message::read_from_socket(this->socket->socket_fd);
        switch (msg.variant) {
        case MSG_EXIT:
            signal(SIGQUIT, SIG_IGN);
            kill(0, SIGQUIT);

            exit(0);
            break;
        case MSG_FAIL:
            perror("Received a fail message");
            exit(1);
            break;
        case MSG_ACK:
            perror("Received an ACK message");
            exit(1);
            break;
        case MSG_DATA:
            perror("Received an data message");
            exit(1);
            break;
        case MSG_STATUS_CHECK:
            perror("Received an status check message");
            exit(1);
            break;
        case MSG_FORK: {
            fork_result_t result;
            int p = fork();

            result.is_fork = p == 0;

            if (p == 0) {
                char *socket_path = (char *)malloc(msg.content.str.len + 1);
                memcpy(socket_path, msg.content.str.ptr, msg.content.str.len);
                socket_path[msg.content.str.len] = 0;

                Socket *result = new Socket(socket_path);

                free(socket_path);

                return result;
            }

            return nullptr;
        }
        default:
            perror("Unknown message variant");
            exit(1);
            break;
        }
    }
}