#ifndef __FORK_CLIENT_H__
#define __FORK_CLIENT_H__

#include <stdint.h>

class Socket {
  public:
    int socket_fd;

    Socket();
    Socket(const char *socket_path);
    
    void await_exit();
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

    void send_input_width(uint32_t width);
    Socket *await_fork();
};

#endif // __FORK_CLIENT_H__