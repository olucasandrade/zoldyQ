import socket
import struct
import asyncio
from dataclasses import dataclass
from typing import Optional, Any, AsyncIterator

import msgpack


@dataclass
class Message:
    id: str
    queue: str
    payload: Any


class ZoldyQ:
    """Synchronous ZoldyQ client."""
    
    def __init__(self, host: str = 'localhost', port: int = 6380, password: Optional[str] = None):
        self.host = host
        self.port = port
        self.password = password
        self.sock: Optional[socket.socket] = None
        self._buffer = b''
    
    def connect(self) -> 'ZoldyQ':
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.connect((self.host, self.port))
        if self.password:
            self._call('auth', password=self.password)
        return self
    
    def close(self):
        if self.sock:
            self.sock.close()
            self.sock = None
    
    def __enter__(self) -> 'ZoldyQ':
        return self.connect()
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
    
    def _send(self, data: dict):
        packed = msgpack.packb(data)
        header = struct.pack('<I', len(packed))
        self.sock.sendall(header + packed)
    
    def _recv(self) -> dict:
        while len(self._buffer) < 4:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise ConnectionError("Connection closed")
            self._buffer += chunk
        
        length = struct.unpack('<I', self._buffer[:4])[0]
        
        while len(self._buffer) < 4 + length:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise ConnectionError("Connection closed")
            self._buffer += chunk
        
        payload = self._buffer[4:4 + length]
        self._buffer = self._buffer[4 + length:]
        return msgpack.unpackb(payload, raw=False)
    
    def _call(self, cmd: str, **kwargs) -> dict:
        request = {'cmd': cmd, **kwargs}
        self._send(request)
        response = self._recv()
        if not response.get('ok'):
            raise Exception(response.get('error', 'Unknown error'))
        return response
    
    def ping(self, message: Optional[str] = None) -> str:
        kwargs = {}
        if message:
            kwargs['payload'] = message
        response = self._call('ping', **kwargs)
        return response.get('pong', 'PONG')
    
    def push(self, queue: str, payload: Any) -> str:
        response = self._call('push', queue=queue, payload=payload)
        return response['id']
    
    def pop(self, queue: str, timeout: int = 0) -> Optional[Message]:
        response = self._call('pop', queue=queue, timeout=timeout)
        if response.get('id'):
            return Message(
                id=response['id'],
                queue=response.get('queue', queue),
                payload=response.get('payload')
            )
        return None
    
    def ack(self, message_id: str):
        self._call('ack', id=message_id)
    
    def nack(self, message_id: str):
        self._call('nack', id=message_id)
    
    def length(self, queue: str) -> int:
        response = self._call('len', queue=queue)
        return response.get('length', 0)
    
    def delete(self, queue: str) -> bool:
        response = self._call('del', queue=queue)
        return response.get('length', 0) > 0


class ZoldyQAsync:
    """Asynchronous ZoldyQ client with subscription support."""
    
    def __init__(self, host: str = 'localhost', port: int = 6380, password: Optional[str] = None):
        self.host = host
        self.port = port
        self.password = password
        self.reader: Optional[asyncio.StreamReader] = None
        self.writer: Optional[asyncio.StreamWriter] = None
        self._buffer = b''
    
    async def connect(self) -> 'ZoldyQAsync':
        self.reader, self.writer = await asyncio.open_connection(self.host, self.port)
        if self.password:
            await self._call('auth', password=self.password)
        return self
    
    async def close(self):
        if self.writer:
            self.writer.close()
            await self.writer.wait_closed()
            self.writer = None
            self.reader = None
    
    async def __aenter__(self) -> 'ZoldyQAsync':
        return await self.connect()
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()
    
    async def _send(self, data: dict):
        packed = msgpack.packb(data)
        header = struct.pack('<I', len(packed))
        self.writer.write(header + packed)
        await self.writer.drain()
    
    async def _recv(self) -> dict:
        while len(self._buffer) < 4:
            chunk = await self.reader.read(4096)
            if not chunk:
                raise ConnectionError("Connection closed")
            self._buffer += chunk
        
        length = struct.unpack('<I', self._buffer[:4])[0]
        
        while len(self._buffer) < 4 + length:
            chunk = await self.reader.read(4096)
            if not chunk:
                raise ConnectionError("Connection closed")
            self._buffer += chunk
        
        payload = self._buffer[4:4 + length]
        self._buffer = self._buffer[4 + length:]
        return msgpack.unpackb(payload, raw=False)
    
    async def _call(self, cmd: str, **kwargs) -> dict:
        request = {'cmd': cmd, **kwargs}
        await self._send(request)
        response = await self._recv()
        if not response.get('ok'):
            raise Exception(response.get('error', 'Unknown error'))
        return response
    
    async def ping(self, message: Optional[str] = None) -> str:
        kwargs = {}
        if message:
            kwargs['payload'] = message
        response = await self._call('ping', **kwargs)
        return response.get('pong', 'PONG')
    
    async def push(self, queue: str, payload: Any) -> str:
        response = await self._call('push', queue=queue, payload=payload)
        return response['id']
    
    async def pop(self, queue: str, timeout: int = 0) -> Optional[Message]:
        response = await self._call('pop', queue=queue, timeout=timeout)
        if response.get('id'):
            return Message(
                id=response['id'],
                queue=response.get('queue', queue),
                payload=response.get('payload')
            )
        return None
    
    async def ack(self, message_id: str):
        await self._call('ack', id=message_id)
    
    async def nack(self, message_id: str):
        await self._call('nack', id=message_id)
    
    async def length(self, queue: str) -> int:
        response = await self._call('len', queue=queue)
        return response.get('length', 0)
    
    async def delete(self, queue: str) -> bool:
        response = await self._call('del', queue=queue)
        return response.get('length', 0) > 0
    
    async def subscribe(self, queue: str) -> AsyncIterator[Message]:
        """Subscribe to a queue and yield messages as they arrive."""
        await self._send({'cmd': 'subscribe', 'queue': queue})
        response = await self._recv()
        if not response.get('ok'):
            raise Exception(response.get('error', 'Subscribe failed'))
        
        while True:
            msg = await self._recv()
            if msg.get('type') == 'message':
                yield Message(
                    id=msg['id'],
                    queue=msg['queue'],
                    payload=msg.get('payload')
                )
    
    async def unsubscribe(self, queue: str):
        await self._call('unsubscribe', queue=queue)
