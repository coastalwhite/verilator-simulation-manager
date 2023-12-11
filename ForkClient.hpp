#ifndef __FORK_CLIENT_H__
#define __FORK_CLIENT_H__

class SocketPair {
  public:
    int sc_socket_fd;
    int cs_socket_fd;

    SocketPair();
    SocketPair(const char *sc_socket_path, const char *cs_socket_path);
    ~SocketPair();
};

struct fork_result_t {
    bool is_fork;
    SocketPair socket_pair;
};

class ForkClient {
  private:
    SocketPair *pair;

  public:
    ForkClient(const int argc, const char **argv);
    ~ForkClient();

    SocketPair *await_fork();
};

#endif // __FORK_CLIENT_H__