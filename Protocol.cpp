#include "Protocol.hpp"
#include "ProtocolUtil.hpp"

#include <cstdio>
#include <cstdlib>
#include <errno.h>
#include <signal.h>
#include <sys/socket.h>
#include <unistd.h>

/// Busy-loop to receive all `n` bytes from file-descriptor `fd` into `dst`.
///
/// If successful, it returns 0. This will return any error that occurs and quit
/// if the parent process does not exist anymore.
ssize_t recv_all(int fd, uint8_t *dst, size_t n) {
    // @Fix. We could block with MSG_PEEK and MSG_WAITALL to remove the busy
    // looping when `t == 0`.

    while (n > 0) {
        ssize_t t = recv(fd, dst, n, 0);

        if (t < 0)
            return t;
        if (t == 0)
            quit_if_no_parent();

        dst += t;
        n -= t;
    }

    return 0;
}

/// Busy-loop to send all `n` bytes from `src` to file-descriptor `fd`.
///
/// If successful, it returns 0. This will return any error that occurs and quit
/// if the parent process does not exist anymore.
ssize_t send_all(int fd, uint8_t *src, size_t n) {
    while (n > 0) {
        ssize_t t = send(fd, src, n, 0);

        if (t < 0)
            return t;
        if (t == 0)
            quit_if_no_parent();

        src += t;
        n -= t;
    }

    return 0;
}

void free_bytearray(bytearray_t bytearray) {
    if (bytearray.do_free) {
        free(bytearray.ptr);
    }
}

Message::Message() : variant(MSG_ACK), content() {}
Message::~Message() {
    switch (this->variant) {
    case (MSG_ACK):
    case (MSG_INPUT_WIDTH):
    case (MSG_NUM_FORKS):
    case (MSG_EXIT):
        break;
    case (MSG_FORK):
		free_bytearray(this->content.fork_info.input);
		free_bytearray(this->content.fork_info.output);
        break;
    case (MSG_FAIL):
    case (MSG_DATA):
		free_bytearray(this->content.bytearray);
        break;
    }
}

uint32_t take_u32(int fd) {
    uint8_t bs[4];

    if (recv_all(fd, bs, 4) < 0) {
        perror("Failed to read 4 bytes for `u32`\n");
        exit(1);
    }

    uint32_t b0 = (uint32_t)bs[0];
    uint32_t b1 = (uint32_t)bs[1];
    uint32_t b2 = (uint32_t)bs[2];
    uint32_t b3 = (uint32_t)bs[3];

    // Little-Endian format
    return (b3 << 24) | (b2 << 16) | (b1 << 8) | b0;
}

void write_u32(int fd, uint16_t n) {
    uint8_t bs[4];

    // Little-Endian format
    bs[0] = (n >> 0) & 0xFF;
    bs[1] = (n >> 8) & 0xFF;
    bs[2] = (n >> 16) & 0xFF;
    bs[3] = (n >> 24) & 0xFF;

    if (send_all(fd, bs, 4) < 0) {
        perror("Failed to write 4 bytes for `u32`\n");
        exit(1);
    }
}

bytearray_t take_bytearray(int fd) {
    bytearray_t bytearray;

    bytearray.len = take_u32(fd);
    bytearray.ptr = (uint8_t *)malloc(bytearray.len);
    bytearray.do_free = true;

    if (bytearray.ptr == NULL) {
        perror("Failed to allocate data for string\n");
        exit(1);
    }

    if (recv_all(fd, bytearray.ptr, bytearray.len) < 0) {
        perror("Failed to read data from socket\n");
        exit(1);
    }

    return bytearray;
}

void write_bytearray(int fd, bytearray_t bytearray) {
    write_u32(fd, bytearray.len);

    if (send_all(fd, bytearray.ptr, bytearray.len) < 0) {
        perror("Failed to write data to socket\n");
        exit(1);
    }
}

Message Message::expect_from_socket(int fd, message_variant_t variant) {
	Message msg = Message::read_from_socket(fd);

	if (msg.variant != variant) {
		fprintf(stderr, "Expected message variant '%i', found '%i'\n", variant, msg.variant);
		exit(1);
	}

	return msg;
}

Message Message::read_from_socket(int fd) {
    uint8_t variant;
    if (recv_all(fd, &variant, 1) < 0) {
        perror("Failed to read variant data from socket\n");
        exit(1);
    }

    Message msg;
    msg.variant = (message_variant_t)variant;

    switch (variant) {
    case MSG_ACK:
    case (MSG_EXIT):
        break;
    case MSG_FAIL:
    case MSG_FORK:
        msg.content.fork_info.input = take_bytearray(fd);
        msg.content.fork_info.output = take_bytearray(fd);
        break;
    case MSG_DATA:
        msg.content.bytearray = take_bytearray(fd);
        break;
    case MSG_INPUT_WIDTH:
    case MSG_NUM_FORKS:
        msg.content.integer = take_u32(fd);
        break;
    default:
        perror("Variant is invalid\n");
        exit(1);
        break;
    }

    return msg;
}

void Message::write_to_socket(int fd) {
    uint8_t variant = (uint8_t)this->variant;
    if (send_all(fd, &variant, 1) < 0) {
        perror("Failed to write variant data to socket\n");
        exit(1);
    }

    switch (this->variant) {
    case (MSG_ACK):
    case (MSG_EXIT):
        break;
        break;
    case (MSG_FORK):
        write_bytearray(fd, this->content.fork_info.input);
        write_bytearray(fd, this->content.fork_info.output);
        break;
    case (MSG_FAIL):
    case (MSG_DATA):
        write_bytearray(fd, this->content.bytearray);
        break;
    case (MSG_INPUT_WIDTH):
    case (MSG_NUM_FORKS):
        write_u32(fd, this->content.integer);
        break;
    }
}

Message Message::ack() {
    Message msg;
    msg.variant = MSG_ACK;
    return msg;
}

Message Message::fail(char *str, uint16_t len) {
    Message msg;
    msg.variant = MSG_FAIL;
    msg.content.bytearray.ptr = (uint8_t*) str;
    msg.content.bytearray.len = len;
	msg.content.bytearray.do_free = false;
    return msg;
}

Message Message::input_width(uint32_t width) {
    Message msg;
    msg.variant = MSG_INPUT_WIDTH;
    msg.content.integer = width;
    return msg;
}

Message Message::data_msg(uint8_t *bytearray, uint32_t len) {
    Message msg;
    msg.variant = MSG_DATA;
    msg.content.bytearray.ptr = bytearray;
    msg.content.bytearray.len = len;
    msg.content.bytearray.do_free = false;
    return msg;
}
