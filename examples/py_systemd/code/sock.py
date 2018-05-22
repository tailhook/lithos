import os
import sys
import socket

assert os.environ["LISTEN_FDS"] == "1"
assert os.environ["LISTEN_FDNAMES"] == "input_port"
assert os.environ["LISTEN_PID"] == str(os.getpid())

sock = socket.socket(fileno=3)
while True:
    s, a = sock.accept()
    s.send(b'hello')
    s.close()

