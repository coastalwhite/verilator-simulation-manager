#include "ForkClient.hpp"

#include "Protocol.hpp"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>

SocketPair::SocketPair() : sc_socket_fd(0), cs_socket_fd(0) {}

SocketPair::SocketPair(const char *sc_socket_path, const char *cs_socket_path) {
    int sc_socket_listener_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    int cs_socket_fd = socket(AF_UNIX, SOCK_STREAM, 0);

    struct sockaddr_un sc_address, cs_address;

    sc_address.sun_family = AF_UNIX;
    cs_address.sun_family = AF_UNIX;

    strcpy(sc_address.sun_path, sc_socket_path);
    strcpy(cs_address.sun_path, cs_socket_path);

	unlink(sc_socket_path);
    if (bind(sc_socket_listener_fd, (struct sockaddr *)&sc_address, sizeof(sc_address)) <
        0) {
        perror("Bind Failed");
        exit(EXIT_FAILURE);
    }

    if (listen(sc_socket_listener_fd, 1) < 0) {
        perror("Listen Failed");
        exit(EXIT_FAILURE);
    }

    if (connect(cs_socket_fd, (struct sockaddr *)&cs_address,
                sizeof(cs_address)) < 0) {
        perror("Connect Failed");
        exit(EXIT_FAILURE);
    }

    socklen_t addr_size = sizeof(sc_address);

    int sc_socket_fd;
    if ((sc_socket_fd = accept(sc_socket_listener_fd, (struct sockaddr *)&sc_address,
                                   &addr_size)) < 0) {
        perror("Accept Failed");
        exit(EXIT_FAILURE);
    }

	close(sc_socket_listener_fd);

    this->sc_socket_fd = sc_socket_fd;
    this->cs_socket_fd = cs_socket_fd;
}

SocketPair::~SocketPair() {
    close(this->sc_socket_fd);
    close(this->cs_socket_fd);
}

ForkClient::ForkClient(const int argc, const char **argv) {
    if (argc < 3) {
        perror("[ERROR]: not enough args");
        exit(2);
    }

    const char *sc_socket_path = argv[1];
    const char *cs_socket_path = argv[2];

    pair = new SocketPair(sc_socket_path, cs_socket_path);
}

ForkClient::~ForkClient() { delete this->pair; }

SocketPair* ForkClient::await_fork() {
    while (1) {
        Message msg = Message::read_from_socket(this->pair->sc_socket_fd);
        switch (msg.variant) {
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
                char *sc_socket_path =
                    (char *)malloc(msg.content.paths[0].len + 1);
                char *cs_socket_path =
                    (char *)malloc(msg.content.paths[1].len + 1);

                memcpy(sc_socket_path, msg.content.paths[0].ptr, msg.content.paths[0].len);
                memcpy(cs_socket_path, msg.content.paths[1].ptr, msg.content.paths[1].len);

                sc_socket_path[msg.content.paths[0].len] = 0;
                cs_socket_path[msg.content.paths[1].len] = 0;

                SocketPair *result_pair = new SocketPair(sc_socket_path, cs_socket_path);

                free(sc_socket_path);
                free(cs_socket_path);

				return result_pair;
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