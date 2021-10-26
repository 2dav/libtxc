#!/bin/python
import socket
from threading import Thread

addr = socket.gethostbyname("127.0.0.1")
port = 5555

connect_cmd = b"""<command id="connect">
  <login></login>
  <password></password>
  <milliseconds>true</milliseconds>
  <autopos>false</autopos>
  <rqdelay>10</rqdelay>
  <host>tr1.finam.ru</host>
  <port>3900</port>
</command>\0"""

print("connecting to proxy server")
cmd = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
cmd.connect((addr, port))

r = cmd.recv(32)
dp = int.from_bytes(r, "little")
print(r)
print("< ", dp)

print("connecting to data stream")
rcv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
rcv.connect((addr, dp))

def rcv_print_loop(rcv):
    while True:
        b = rcv.recv(1 << 20).decode()
        if b == b'':
            break
        print(b)

Thread(target=rcv_print_loop, args=(rcv, )).start()

print("sending 'connect' command")
cmd.sendall(connect_cmd)
print("< ", cmd.recv(256))
