#ifndef __PROTOCOL_HPP__
#define __PROTOCOL_HPP__

#include <cstddef>
#include <stdint.h>

enum message_variant_t {
    MSG_FAIL = 0,
    MSG_ACK  = 1,
    MSG_STATUS_CHECK = 2,

    MSG_FORK = 16,
    MSG_DATA = 17,
};

struct data_bytearray_t {
    uint32_t len;
    uint8_t* ptr;
};

struct data_str_t {
    uint16_t len;
    char* ptr;
};

union message_data_t {
    data_bytearray_t bytearray;
    data_str_t str;
    data_str_t paths[2];

	message_data_t() {
		this->bytearray.len = 0;
		this->bytearray.ptr = (uint8_t*) 0;
	}
};

class Message {
public:
    message_variant_t variant;
    message_data_t content;

    Message();
    ~Message();

	static Message read_from_socket(int fd);
	void write_to_socket(int fd);

    static Message ack();
    static Message fail(char *str, uint16_t len);
    static Message status_check();
    static Message data_msg(uint8_t *bytearray, uint32_t len);
};

#endif // __PROTOCOL_HPP__