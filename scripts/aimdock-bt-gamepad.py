#!/usr/bin/env python3
"""
AimDock Bluetooth HID Gamepad Daemon
Emulates a Bluetooth HID gamepad for CODM
Run with: sudo python3 scripts/aimdock-bt-gamepad.py
"""

import socket
import struct
import threading
import time
import subprocess
import sys

BT_ADDR = "C0:B5:D7:13:07:BC"
HID_CONTROL_PSM = 17
HID_INTERRUPT_PSM = 19
GAMEPAD_UDP_PORT = 7071

# Standard HID gamepad descriptor
# 4 axes (LX, LY, RX, RY) 0-255 each + 16 buttons
HID_DESCRIPTOR = bytes([
    0x05, 0x01,        # Usage Page (Generic Desktop)
    0x09, 0x05,        # Usage (Game Pad)
    0xA1, 0x01,        # Collection (Application)
    0xA1, 0x00,        #   Collection (Physical)
    0x05, 0x01,        #     Usage Page (Generic Desktop)
    0x09, 0x30,        #     Usage (X)  - Left stick X
    0x09, 0x31,        #     Usage (Y)  - Left stick Y
    0x09, 0x32,        #     Usage (Z)  - Right stick X
    0x09, 0x35,        #     Usage (Rz) - Right stick Y
    0x15, 0x00,        #     Logical Minimum (0)
    0x26, 0xFF, 0x00,  #     Logical Maximum (255)
    0x75, 0x08,        #     Report Size (8)
    0x95, 0x04,        #     Report Count (4)
    0x81, 0x02,        #     Input (Data, Var, Abs)
    0x05, 0x09,        #     Usage Page (Button)
    0x19, 0x01,        #     Usage Minimum (Button 1)
    0x29, 0x10,        #     Usage Maximum (Button 16)
    0x15, 0x00,        #     Logical Minimum (0)
    0x25, 0x01,        #     Logical Maximum (1)
    0x75, 0x01,        #     Report Size (1)
    0x95, 0x10,        #     Report Count (16)
    0x81, 0x02,        #     Input (Data, Var, Abs)
    0xC0,              #   End Collection
    0xC0               # End Collection
])

# Shared gamepad state
state = {'lx': 128, 'ly': 128, 'rx': 128, 'ry': 128, 'buttons': 0}
state_lock = threading.Lock()
intr_client = None
intr_lock = threading.Lock()

def build_report():
    with state_lock:
        return struct.pack('<BBBBBH',
            0xA1,
            state['lx'], state['ly'],
            state['rx'], state['ry'],
            state['buttons']
        )

def udp_receiver():
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(('127.0.0.1', GAMEPAD_UDP_PORT))
    print(f"Listening for gamepad state on UDP {GAMEPAD_UDP_PORT}")
    while True:
        try:
            data, _ = sock.recvfrom(8)
            if len(data) >= 6:
                with state_lock:
                    state['lx'] = data[0]
                    state['ly'] = data[1]
                    state['rx'] = data[2]
                    state['ry'] = data[3]
                    state['buttons'] = struct.unpack('<H', data[4:6])[0]
        except Exception as e:
            print(f"UDP error: {e}")

def report_sender():
    while True:
        time.sleep(1/60)
        with intr_lock:
            client = intr_client
        if client:
            try:
                client.send(build_report())
            except Exception:
                pass

def register_sdp():
    descriptor_hex = ''.join(f'{b:02x}' for b in HID_DESCRIPTOR)
    xml = f"""<?xml version="1.0" encoding="UTF-8"?>
<record>
  <attribute id="0x0001"><sequence><uuid value="0x1124"/></sequence></attribute>
  <attribute id="0x0004">
    <sequence>
      <sequence><uuid value="0x0100"/><uint16 value="0x0011"/></sequence>
      <sequence><uuid value="0x0011"/></sequence>
    </sequence>
  </attribute>
  <attribute id="0x0005"><sequence><uuid value="0x1002"/></sequence></attribute>
  <attribute id="0x0009">
    <sequence><sequence><uuid value="0x1124"/><uint16 value="0x0100"/></sequence></sequence>
  </attribute>
  <attribute id="0x000d">
    <sequence>
      <sequence>
        <sequence><uuid value="0x0100"/><uint16 value="0x0013"/></sequence>
        <sequence><uuid value="0x0011"/></sequence>
      </sequence>
    </sequence>
  </attribute>
  <attribute id="0x0100"><text value="AimDock Controller"/></attribute>
  <attribute id="0x0101"><text value="AimDock"/></attribute>
  <attribute id="0x0102"><text value="AimDock"/></attribute>
  <attribute id="0x0200"><uint16 value="0x0100"/></attribute>
  <attribute id="0x0201"><uint16 value="0x0111"/></attribute>
  <attribute id="0x0202"><uint8 value="0x40"/></attribute>
  <attribute id="0x0203"><uint8 value="0x00"/></attribute>
  <attribute id="0x0204"><boolean value="false"/></attribute>
  <attribute id="0x0205"><boolean value="false"/></attribute>
  <attribute id="0x0206">
    <sequence><sequence>
      <uint8 value="0x22"/>
      <text encoding="hex" value="{descriptor_hex}"/>
    </sequence></sequence>
  </attribute>
  <attribute id="0x020b"><uint16 value="0x0100"/></attribute>
  <attribute id="0x020c"><uint16 value="0x0c80"/></attribute>
  <attribute id="0x020d"><boolean value="false"/></attribute>
  <attribute id="0x020e"><boolean value="false"/></attribute>
  <attribute id="0x020f"><uint16 value="0x0640"/></attribute>
  <attribute id="0x0210"><uint16 value="0x0320"/></attribute>
</record>"""
    with open('/tmp/aimdock-hid.xml', 'w') as f:
        f.write(xml)
    subprocess.run(['sdptool', 'del', '0x00005678'], capture_output=True)
    result = subprocess.run(
        ['sdptool', 'add', '--handle=0x00005678', f'--xml=/tmp/aimdock-hid.xml'],
        capture_output=True, text=True
    )
    if result.returncode == 0:
        print("SDP record registered")
    else:
        print(f"SDP warning: {result.stderr.strip()}")

def make_discoverable():
    subprocess.run(['bluetoothctl', 'discoverable', 'on'], capture_output=True)
    subprocess.run(['bluetoothctl', 'pairable', 'on'], capture_output=True)
    print("Laptop is now discoverable as 'AimDock Controller'")
    print("Go to CODM controller settings and connect via Bluetooth")

def hid_server():
    global intr_client
    ctrl_sock = socket.socket(socket.AF_BLUETOOTH, socket.SOCK_SEQPACKET, socket.BTPROTO_L2CAP)
    ctrl_sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    ctrl_sock.bind((BT_ADDR, HID_CONTROL_PSM))
    ctrl_sock.listen(1)

    intr_sock = socket.socket(socket.AF_BLUETOOTH, socket.SOCK_SEQPACKET, socket.BTPROTO_L2CAP)
    intr_sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    intr_sock.bind((BT_ADDR, HID_INTERRUPT_PSM))
    intr_sock.listen(1)

    print("Waiting for phone to connect...")

    while True:
        ctrl_client, ctrl_addr = ctrl_sock.accept()
        print(f"Control channel connected: {ctrl_addr[0]}")

        client, addr = intr_sock.accept()
        print(f"Interrupt channel connected: {addr[0]}")
        print("Gamepad active! Mouse = camera, WASD = movement")

        with intr_lock:
            intr_client = client

        # Keep control channel alive
        try:
            while True:
                data = ctrl_client.recv(128)
                if not data:
                    break
        except Exception:
            pass

        with intr_lock:
            intr_client = None
        print("Phone disconnected. Waiting for reconnect...")

if __name__ == '__main__':
    print("AimDock BT HID Gamepad Daemon starting...")
    register_sdp()
    make_discoverable()
    threading.Thread(target=udp_receiver, daemon=True).start()
    threading.Thread(target=report_sender, daemon=True).start()
    hid_server()
