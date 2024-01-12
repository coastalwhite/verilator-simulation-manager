#include "ProtocolUtil.hpp"

#include <cstdlib>
#include <cstdio>
#include <csignal>

void quit_if_no_parent() {
    // As per the kill(2) MAN page:
    // > If sig is 0, then no signal is sent, but existence and permission
    // >  checks are still performed; this can be used to check for the
    // >  existence of a process ID or process group ID that the caller is
    // >  permitted to signal.
    //
    // Here, we also crash on `EPERM`, but this is okay.
    if (kill(getppid(), 0) < 0) {
        perror("While polling the socket, the parent process became "
               "unavailable.\n");
        exit(0);
    }
}