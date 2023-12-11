#include "ForkClient.hpp"
#include "Protocol.hpp"
#include <cstdio>
#include <unistd.h>

int main(const int argc, const char** argv) {
    ForkClient client(argc, argv);

	int i = 0;
	while (1) {
		i++;

		SocketPair *result = client.await_fork();

		if (result != nullptr) {
            Message msg = Message::read_from_socket(result->sc_socket_fd);

			printf("Fork %i: Hello World!\n", i);

			delete result;
			return 0;
		}
	}

    return 0;
}