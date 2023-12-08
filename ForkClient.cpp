#include "ForkClient.hpp"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/types.h>
#include <unistd.h>

SocketPair::SocketPair(
    const char* sc_socket_path,
    const char* cs_socket_path
) {
    int sc_socket_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    int cs_socket_fd = socket(AF_UNIX, SOCK_STREAM, 0);

    struct sockaddr_un sc_address, cs_address;

    sc_address.sun_family = AF_UNIX;
    cs_address.sun_family = AF_UNIX;

    strcpy(sc_address.sun_path, sc_socket_path);
    strcpy(cs_address.sun_path, cs_socket_path);

    if (bind(sc_socket_fd, (struct sockaddr*)&sc_address, sizeof(sc_address)) < 0) {
        perror("Bind Failed");
        exit(EXIT_FAILURE);
    }

    if (listen(sc_socket_fd, 1) < 0) {
        perror("Listen Failed");
        exit(EXIT_FAILURE);
    }

    if (connect(cs_socket_fd, (struct sockaddr*)&cs_address, sizeof(cs_address)) < 0) {
        perror("Connect Failed");
        exit(EXIT_FAILURE);
    }


    socklen_t addr_size = sizeof(sc_address);

    int new_sc_socket_fd;
    if ((new_sc_socket_fd = accept(sc_socket_fd, (struct sockaddr*)&sc_address, &addr_size)) < 0) {
        perror("Accept Failed");
        exit(EXIT_FAILURE);
    }

    this->sc_socket_fd = new_sc_socket_fd;
    this->cs_socket_fd = cs_socket_fd;
}

SocketPair::~SocketPair() {
    close(this->sc_socket_fd);
    close(this->cs_socket_fd);
}

ForkClient::ForkClient(const int argc, const char** argv) {
    if (argc < 3) {
        perror("[ERROR]: not enough args");
        exit(2);
    }

    const char* sc_socket_path = argv[1];
    const char* cs_socket_path = argv[2];

    pair = new SocketPair(sc_socket_path, cs_socket_path);
}

ForkClient::~ForkClient() {
    delete this->pair;
}

int ForkClient::await_fork(SocketPair* _fork_pair) {
    return 0;
}