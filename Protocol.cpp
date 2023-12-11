#include "Protocol.hpp"

#include <cstdio>
#include <cstdlib>
#include <sys/socket.h>
#include <unistd.h>

Message::Message() : variant(MSG_ACK), content() {}
Message::~Message() {
    switch (this->variant) {
    case (MSG_ACK):
    case (MSG_STATUS_CHECK):
        break;
    case (MSG_FAIL):
        free(this->content.str.ptr);
        break;
    case (MSG_FORK):
        free(this->content.paths[0].ptr);
        free(this->content.paths[1].ptr);
        break;
    case (MSG_DATA):
        free(this->content.bytearray.ptr);
        break;
    }
}

data_str_t take_string(int fd) {
    uint8_t bs[2];

    if (recv(fd, bs, 2, 0) < 0) {
        perror("Read error");
        exit(1);
    }

    size_t hb = (size_t)bs[1];
    size_t lb = (size_t)bs[0];

    data_str_t str;

    str.len = (hb << 8) | lb;
    str.ptr = (char *)malloc(str.len);

    if (str.ptr == NULL) {
        perror("Failed to allocate");
        exit(1);
    }

    if (recv(fd, str.ptr, str.len, 0) < 0) {
        perror("Read error");
        exit(1);
    }

    return str;
}

data_bytearray_t take_bytearray(int fd) {
    uint8_t bs[4];

    if (recv(fd, bs, 4, 0) < 0) {
        perror("Read error");
        exit(1);
    }

    size_t b3 = (size_t)bs[3];
    size_t b2 = (size_t)bs[2];
    size_t b1 = (size_t)bs[1];
    size_t b0 = (size_t)bs[0];

    data_bytearray_t bytearray;

    bytearray.len = (b3 << 24) | (b2 << 16) | (b1 << 8) | b0;
    bytearray.ptr = (uint8_t *)malloc(bytearray.len);

    if (bytearray.ptr == NULL) {
        perror("Failed to allocate");
        exit(1);
    }

    if (recv(fd, bytearray.ptr, bytearray.len, 0) < 0) {
        perror("Read error");
        exit(1);
    }

    return bytearray;
}

void write_str(int fd, data_str_t str) {
    uint8_t bs[2];

    bs[0] = (str.len >> 0) & 0xFF;
    bs[1] = (str.len >> 8) & 0xFF;

    if (send(fd, bs, 2, 0) < 0) {
        perror("Write error");
        exit(1);
    }

    if (send(fd, str.ptr, str.len, 0) < 0) {
        perror("Write error");
        exit(1);
    }
}

void write_bytearray(int fd, data_bytearray_t bytearray) {
    uint8_t bs[4];

    bs[0] = (bytearray.len >> 0) & 0xFF;
    bs[1] = (bytearray.len >> 8) & 0xFF;
    bs[2] = (bytearray.len >> 16) & 0xFF;
    bs[3] = (bytearray.len >> 24) & 0xFF;

    if (send(fd, bs, 4, 0) < 0) {
        perror("Write error");
        exit(1);
    }

    if (send(fd, bytearray.ptr, bytearray.len, 0) < 0) {
        perror("Write error");
        exit(1);
    }
}

Message Message::read_from_socket(int fd) {
    uint8_t variant;
    if (recv(fd, &variant, 1, 0) < 0) {
        perror("Read error");
        exit(1);
    }

    Message msg;
    msg.variant = (message_variant_t)variant;

    switch (variant) {
    case MSG_ACK:
    case MSG_STATUS_CHECK:
        break;
    case MSG_FAIL:
        msg.content.str = take_string(fd);
        break;
    case MSG_FORK:
        msg.content.paths[0] = take_string(fd);
        msg.content.paths[1] = take_string(fd);
        break;
    case MSG_DATA:
        msg.content.bytearray = take_bytearray(fd);
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
        write_str(fd, this->content.str);
        break;
    case (MSG_FORK):
        perror("Client cannot send fork requests to server.");
        exit(1);
        break;
    case (MSG_DATA):
        write_bytearray(fd, this->content.bytearray);
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
    msg.content.str.ptr = str;
    msg.content.str.len = len;
    return msg;
}

Message Message::status_check() {
    Message msg;
    msg.variant = MSG_STATUS_CHECK;
    return msg;
}

Message Message::data_msg(uint8_t *bytearray, uint32_t len) {
    Message msg;
    msg.variant = MSG_DATA;
    msg.content.bytearray.ptr = bytearray;
    msg.content.bytearray.len = len;
    return msg;
}
