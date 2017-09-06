import sys
import socket

print("VER",sys.version)

sock = socket.socket(fileno=3)
while True:
    s, a = sock.accept()
    s.write(b'hello')
    s.close()

