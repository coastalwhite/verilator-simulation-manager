#ifndef __FORK_CLIENT_H__
#define __FORK_CLIENT_H__

class SocketPair {
private:
    int sc_socket_fd;
    int cs_socket_fd;

public:
    SocketPair(const char* sc_socket_path, const char* cs_socket_path);
    ~SocketPair();
};

class ForkClient {
private:
    SocketPair* pair;

public:
    ForkClient(const int argc, const char** argv);
    ~ForkClient();

    int await_fork(SocketPair* fork_pair);
};

#endif // __FORK_CLIENT_H__