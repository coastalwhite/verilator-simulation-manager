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
        perror("Failed to connect to socket");
        exit(EXIT_FAILURE);
    }

    this->socket_fd = socket_fd;
}

ForkClient::ForkClient(const int argc, const char **argv) {
    if (argc < 2) {
        perror("ForkClient expects the first argument to be the socket path");
        exit(2);
    }

    const char *socket_path = argv[1];

    this->socket = new Socket(socket_path);
}

ForkClient::~ForkClient() { delete this->socket; }

void ForkClient::send_input_width(uint32_t width) {
    Message::input_width(width).write_to_socket(this->socket->socket_fd);
}

void Socket::clean_exit() {
    if (shutdown(this->socket_fd, SHUT_WR) != 0) {
        perror("Failed to shutdown socket");
        exit(1);
    }

    raise(SIGQUIT);
}

Socket *ForkClient::await_fork() {
    Message msg = Message::read_from_socket(this->socket->socket_fd);
    switch (msg.variant) {
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
    case MSG_EXIT:
		printf("Received the EXIT message, exiting...\n");
		this->socket->clean_exit();
    case MSG_FAIL:
        perror("Expected FORK message, received FAIL message.");
        break;
    case MSG_ACK:
        perror("Expected FORK message, received ACK message.");
        break;
    case MSG_DATA:
        perror("Expected FORK message, received DATA message.");
        break;
    default:
        perror("Expected FORK message, received unknown message variant.");
        break;
    }

    exit(1);
}