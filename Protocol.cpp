#include "Protocol.hpp"

#include <cstdio>
#include <cstdlib>

Message::Message() : variant(MSG_ACK), data_size(0), content() {}
Message::~Message() {
    switch (this->variant) {
    case (MSG_ACK):
    case (MSG_STATUS_CHECK):
        break;
    case (MSG_FAIL):
        free(this->content.str);
        break;
    case (MSG_FORK):
        free(this->content.paths[0]);
        free(this->content.paths[1]);
        break;
    case (MSG_DATA):
        free(this->content.bytearray);
        break;
    }
}

char* take_string(int fd, size_t* len) {
	uint8_t bs[2];

	if (read(fd, bs, 2) < 0) {
		perror("Read error");
		exit(1);
	}

	size_t hb = (size_t) bs[1];
	size_t lb = (size_t) bs[0];

	*len = (hb << 8) | lb;

	char *buf = (char*) malloc(*len);

	if (buf == NULL) {
		perror("Failed to allocate");
		exit(1);
	}

	if (read(fd, buf, *len) < 0) {
		perror("Read error");
		exit(1);
	}

	return buf;
}

uint8_t* take_data(int fd, size_t* len) {
	uint8_t bs[4];

	if (read(fd, bs, 4) < 0) {
		perror("Read error");
		exit(1);
	}

	size_t b3 = (size_t) bs[3];
	size_t b2 = (size_t) bs[2];
	size_t b1 = (size_t) bs[1];
	size_t b0 = (size_t) bs[0];

	*len = (b3 << 24) | (b2 << 16) | (b1 << 8) | b0;

	uint8_t *buf = (uint8_t*) malloc(*len);

	if (buf == NULL) {
		perror("Failed to allocate");
		exit(1);
	}

	if (read(fd, buf, *len) < 0) {
		perror("Read error");
		exit(1);
	}

	return buf;
}

Message Message::read_from_socket(int fd) {
    uint8_t variant;
    if (read(fd, &variant, 1) < 0) {
        perror("Read error");
        exit(1);
    }

    Message msg;
    msg.variant = (message_variant_t)variant;

    switch (variant) {
    case (MSG_ACK):
    case (MSG_STATUS_CHECK):
        break;
    case (MSG_FAIL):
		size_t len = 0;
		char* content = take_string(fd, &len);

		msg.data_size = len;
		msg.content.str = content;

        break;
    case (MSG_FORK):
		size_t path0_len = 0;
		char* path0 = take_string(fd, &len);
		size_t path1_len = 0;
		char* path1 = take_string(fd, &len);

		// TODO: Properly set the lengths

        break;
    case (MSG_DATA):
        if (send(fd, this->content.bytearray, this->data_size, 0) < 0) {
            perror("Send error");
            exit(1);
        }
        break;
    default:
        perror("Invalid variant");
        exit(1);
        break;
    }

    return msg;
}

void Message::write_to_socket(int fd) {
    uint8_t variant = (uint8_t)this->variant;
    if (send(fd, &variant, 1, 0) < 0) {
        perror("Send error");
        exit(1);
    }

    switch (this->variant) {
    case (MSG_ACK):
    case (MSG_STATUS_CHECK):
        break;
    case (MSG_FAIL):
        if (send(fd, this->content.str, this->data_size, 0) < 0) {
            perror("Send error");
            exit(1);
        }
        break;
    case (MSG_FORK):
        perror("Client cannot send fork requests to server.");
        exit(1);
        break;
    case (MSG_DATA):
        if (send(fd, this->content.bytearray, this->data_size, 0) < 0) {
            perror("Send error");
            exit(1);
        }
        break;
    }
}

Message Message::ack() {
    Message msg;
    msg.variant = MSG_ACK;
    return msg;
}

Message Message::fail(char *str, size_t len) {
    Message msg;
    msg.variant = MSG_FAIL;
    msg.data_size = len;
    msg.content.str = str;
    return msg;
}

Message Message::status_check() {
    Message msg;
    msg.variant = MSG_STATUS_CHECK;
    return msg;
}

Message Message::data_msg(uint8_t *bytearray, size_t len) {
    Message msg;
    msg.variant = MSG_DATA;
    msg.data_size = len;
    msg.content.bytearray = bytearray;
    return msg;
}
