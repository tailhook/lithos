import sys
import socket

print("VER",sys.version)

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.bind(('0.0.0.0', 80))
sock.listen()
while True:
    s, a = sock.accept()
    s.send(b'hello')
    s.close()

