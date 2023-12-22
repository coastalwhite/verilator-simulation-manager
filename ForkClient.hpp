#ifndef __FORK_CLIENT_H__
#define __FORK_CLIENT_H__

class Socket {
  public:
    int socket_fd;

    Socket();
    Socket(const char *socket_path);
};

struct fork_result_t {
    bool is_fork;
    Socket socket;
};

class ForkClient {
  private:
    Socket *socket;

  public:
    ForkClient(const int argc, const char **argv);
    ~ForkClient();

    Socket *await_fork();
};

#endif // __FORK_CLIENT_H__