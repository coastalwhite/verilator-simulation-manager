#!/usr/bin/env python

import enum
import sys
import socket
import os
import time

class SocketPathPair:
    def __init__(self, sc_socket_path: str, cs_socket_path: str) -> None:
        self.sc_socket_path = sc_socket_path
        self.cs_socket_path = cs_socket_path

    @staticmethod
    def take_argv():
        sc_socket_path = sys.argv[1]
        cs_socket_path = sys.argv[2]

        sc_socket_path = sc_socket_path.rstrip()
        cs_socket_path = cs_socket_path.rstrip()

        return SocketPathPair(sc_socket_path, cs_socket_path)

def eprint(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)

def stdout_print(*args, **kwargs):
    print(*args, file=sys.stdout, end='', **kwargs)
    sys.stdout.flush()

class MessageVariant(enum.IntEnum):
    FAIL = 0
    ACK = 1
    STATUS_CHECK = 2

    FORK = 16
    DATA = 17

def read_str_from_socket(socket) -> str:
    len = socket.recv(2)
    len = int.from_bytes(len, byteorder='little')

    s = socket.recv(len)
    s = s.decode('utf-8')

    return s

def write_str_to_socket(socket, s: str):
    bs = s.encode('utf-8')

    if len(bs) >= (1 << 16):
        raise Exception("String is too large")

    socket.send(len(bs).to_bytes(2, byteorder='little'))
    socket.send(bs)

def read_vec_from_socket(socket) -> bytearray:
    len = socket.recv(4)
    len = int.from_bytes(len, byteorder='little')

    s = socket.recv(len)

    return s

def write_vec_to_socket(socket, s: bytearray):
    if len(s) >= (1 << 32):
        raise Exception("Vec is too large")

    socket.send(len(s).to_bytes(4, byteorder='little'))
    socket.send(s)

class Message:
    def __init__(self, variant: MessageVariant, data = None | str | bytearray | SocketPathPair) -> None:
        self.variant = variant
        self.data = data

    def write_to_socket(self, socket):
        if self.variant.value >= (1 << 8):
            raise Exception("Invalid Variant")

        socket.send(self.variant.value.to_bytes(1))

        match self.variant:
            case MessageVariant.ACK: pass
            case MessageVariant.STATUS_CHECK: pass

            case MessageVariant.FAIL:
                write_str_to_socket(socket, str(self.data))
            case MessageVariant.FORK:
                raise Exception("Cannot send a FORK message from client to server")
            case MessageVariant.DATA:
                write_vec_to_socket(socket, bytearray(self.data))

    @staticmethod
    def read_from_socket(socket):
        variant = socket.recv(1)[0]

        variant = MessageVariant(variant)
        data = None

        match variant:
            case MessageVariant.ACK: pass
            case MessageVariant.STATUS_CHECK: pass

            case MessageVariant.FAIL:
                data = read_str_from_socket(socket)
            case MessageVariant.FORK:
                sc_socket_path = read_str_from_socket(socket)
                cs_socket_path = read_str_from_socket(socket)

                data = SocketPathPair(
                    sc_socket_path,
                    cs_socket_path,
                )
            case MessageVariant.DATA:
                data = read_vec_from_socket(socket)

        return Message(variant, data)

def try_remove_socket(path: str):
    try:
        os.unlink(path)
    except OSError:
        if os.path.exists(path):
            raise

class SocketPair:
    def __init__(self, path_pair: SocketPathPair) -> None:
        self.path_pair = path_pair

        try_remove_socket(path_pair.sc_socket_path)
        sc_socket_listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sc_socket_listener.bind(path_pair.sc_socket_path)
        sc_socket_listener.listen(1)

        cs_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        cs_socket.connect(path_pair.cs_socket_path)

        sc_socket, _ = sc_socket_listener.accept()

        sc_socket.setblocking(True)
        cs_socket.setblocking(True)

        self.sc_socket = sc_socket
        self.cs_socket = cs_socket

    def __del__(self):
        if hasattr(self, "sc_socket"):
            self.sc_socket.close()
            try_remove_socket(self.path_pair.sc_socket_path)

        if hasattr(self, "cs_socket"):
            self.cs_socket.close()
            try_remove_socket(self.path_pair.cs_socket_path)


def main():
    socket_path_pair = SocketPathPair.take_argv()
    socket_pair = SocketPair(socket_path_pair)

    try:
        # receive data from the client
        while True:
            msg = Message.read_from_socket(socket_pair.sc_socket)

            match msg.variant:
                case MessageVariant.ACK:
                    print(f"Got an unexpected ACK message!")
                case MessageVariant.STATUS_CHECK:
                    print(f"Got an unexpected STATUS_CHECK message!")

                case MessageVariant.FAIL:
                    print(f"Got a FAIL message! {msg.data}")

                case MessageVariant.FORK:
                    sc_socket_path = msg.data.sc_socket_path
                    cs_socket_path = msg.data.cs_socket_path

                    processid = os.fork()

                    if processid == 0:
                        fork_socket_pair = SocketPair(SocketPathPair(
                            sc_socket_path,
                            cs_socket_path,
                        ))

                        try:
                            msg = Message.read_from_socket(fork_socket_pair.sc_socket)

                            print([hex(x) for x in list(msg.data)])

                            while True:
                                print("Still alive...")
                                time.sleep(1)
                                pass
                        finally:
                            del fork_socket_pair
    finally:
        del socket_pair

if __name__ == '__main__':
    main()