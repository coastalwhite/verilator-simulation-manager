#ifndef __SHARED_MEMORY_HPP__
#define __SHARED_MEMORY_HPP__

#include <stdint.h>
#include <string.h>

typedef enum {
	ACCESS_READONLY,
	ACCESS_READWRITE,
} access_t;

class SharedMemory {
private:
	int fd;
    const char* id; 
    void* data;
    size_t len;

public:
    SharedMemory(const char* id, access_t access, const size_t len);
    ~SharedMemory();

    void* get_data();
    size_t get_len();
};

#endif // __SHARED_MEMORY_HPP__