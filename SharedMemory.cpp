#include "SharedMemory.hpp"

#include <cstdio>
#include <fcntl.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <unistd.h>

SharedMemory::SharedMemory(const char *id, access_t access, size_t size)
    : id(id) {

    int oflag = 0;

    switch (access) {
    case (ACCESS_READONLY):
        oflag |= O_RDONLY;
        break;
    case (ACCESS_READWRITE):
        oflag |= O_RDWR;
        break;
    }

    // User has read and write permissions
    mode_t mode = S_IRUSR | S_IWUSR;

    int fd = shm_open(id, oflag, mode);

    if (fd < 0) {
        perror("Failed to open shared memory");
        exit(1);
    }

    this->fd = fd;

    int result = ftruncate(fd, size);

    if (result < 0) {
        perror("Failed to truncate shared memory");
        exit(1);
    }

    void *data = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);

    if ((ssize_t)data == -1) {
        perror("Failed to mmap shared memory");
        exit(1);
    }

    this->data = data;
}
SharedMemory::~SharedMemory() { close(this->fd); }

void *SharedMemory::get_data() { return this->data; }
