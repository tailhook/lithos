import os
import sys
import socket

assert os.environ["LISTEN_FDS"] == "1"
assert os.environ["LISTEN_NAMES"] == "input_port"

sock = socket.socket(fileno=3)
while True:
    s, a = sock.accept()
    s.send(b'hello')
    s.close()

