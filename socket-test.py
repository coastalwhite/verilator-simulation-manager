#!/usr/bin/env python

import sys
import socket
import os
import time

def eprint(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)

def stdout_print(*args, **kwargs):
    print(*args, file=sys.stdout, end='', **kwargs)
    sys.stdout.flush()

def main():
    eprint("[FORKER]: Waiting for Hello")
    hello_line = sys.stdin.readline()

    if hello_line.rstrip() != "Hello Forker!":
        eprint("[FORKER]: Invalid Hello Message!")
        exit(1)

    stdout_print("Hello Manager!\n")
    eprint("[FORKER]: Waiting for Sockets")

    mgr_forker_sock_path = sys.stdin.readline()
    forker_mgr_sock_path = sys.stdin.readline()

    mgr_forker_sock_path = mgr_forker_sock_path.rstrip()
    forker_mgr_sock_path = forker_mgr_sock_path.rstrip()

    # remove the socket file if it already exists
    try:
        os.unlink(mgr_forker_sock_path)
    except OSError:
        if os.path.exists(mgr_forker_sock_path):
            raise

    mgr_forker_sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    mgr_forker_sock.bind(mgr_forker_sock_path)
    mgr_forker_sock.listen(1)

    forker_mgr_sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    forker_mgr_sock.connect(forker_mgr_sock_path)

    stdout_print("Received Sockets!\n")

    # accept connections
    connection, client_address = mgr_forker_sock.accept()

    try:
        # receive data from the client
        while True:
            cmd = connection.recv(1)

            match cmd[0]:
                case 1: # Fork
                    # ACK
                    forker_mgr_sock.send(bytearray([42]))

                    mgr_client_sock_path_size = connection.recv(4)
                    client_mgr_sock_path_size = connection.recv(4)

                    mgr_client_sock_path_size = int.from_bytes(mgr_client_sock_path_size, byteorder='big')
                    client_mgr_sock_path_size = int.from_bytes(client_mgr_sock_path_size, byteorder='big')

                    mgr_client_sock_path = str(connection.recv(mgr_client_sock_path_size), 'utf-8')
                    client_mgr_sock_path = str(connection.recv(client_mgr_sock_path_size), 'utf-8')

                    mgr_client_sock_path = mgr_client_sock_path.rstrip()
                    client_mgr_sock_path = client_mgr_sock_path.rstrip()

                    processid = os.fork()

                    if processid == 0:
                        # remove the socket file if it already exists
                        try:
                            os.unlink(mgr_client_sock_path)
                        except OSError:
                            if os.path.exists(mgr_client_sock_path):
                                raise

                        eprint(f"[CHILD]: '{mgr_client_sock_path}'")
                        eprint(f"[CHILD]: '{client_mgr_sock_path}'")

                        eprint("[CHILD]: Setting up socket MGR->Client")
                        mgr_client_sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                        mgr_client_sock.bind(mgr_client_sock_path)
                        mgr_client_sock.listen(1)

                        eprint("[CHILD]: Setting up socket Client->MGR")
                        client_mgr_sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                        client_mgr_sock.connect(client_mgr_sock_path)

                        # accept connections
                        banaan, banaan_address = mgr_client_sock.accept()

                        while True:
                            pass

                    # NOTE: This is a temporary hack, but can be solved in C++ by having shared memory
                    time.sleep(1)

                    # ACK
                    forker_mgr_sock.send(bytearray([42]))
    finally:
        # close the connection
        connection.close()
        # remove the socket file
        os.unlink(mgr_forker_sock_path)

    sys.stdin.readline()

if __name__ == '__main__':
    main()