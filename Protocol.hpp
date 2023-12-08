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

union message_data_t {
    uint8_t* bytearray;
    char* str;
    char* paths[2];

	message_data_t() {
		this->str = (char*) 0;
	}
};

class Message {
public:
    message_variant_t variant;
	size_t data_size;
    message_data_t content;

    Message();
    ~Message();

	static Message read_from_socket(int fd);
	void write_to_socket(int fd);

    static Message ack();
    static Message fail(char *str, size_t len);
    static Message status_check();
    static Message data_msg(uint8_t *bytearray, size_t len);
};

#endif // __PROTOCOL_HPP__