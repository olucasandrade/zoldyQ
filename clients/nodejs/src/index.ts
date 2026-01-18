import * as net from 'net';
import { encode, decode } from '@msgpack/msgpack';
import { EventEmitter } from 'events';

export interface Message {
  id: string;
  queue: string;
  payload: any;
}

interface Response {
  ok: boolean;
  error?: string;
  id?: string;
  payload?: any;
  queue?: string;
  length?: number;
  pong?: string;
  type?: string;
}

interface ZoldyQOptions {
  host?: string;
  port?: number;
  password?: string;
}

export class ZoldyQ extends EventEmitter {
  private host: string;
  private port: number;
  private password?: string;
  private socket: net.Socket | null = null;
  private buffer: Buffer = Buffer.alloc(0);
  private pending: Array<{
    resolve: (value: Response) => void;
    reject: (error: Error) => void;
  }> = [];
  private connected = false;
  private subscriptions: Set<string> = new Set();

  constructor(options: ZoldyQOptions = {}) {
    super();
    this.host = options.host || 'localhost';
    this.port = options.port || 6380;
    this.password = options.password;
  }

  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.socket = net.createConnection({ host: this.host, port: this.port });

      this.socket.on('connect', async () => {
        this.connected = true;
        try {
          if (this.password) {
            await this._call({ cmd: 'auth', password: this.password });
          }
          resolve();
        } catch (err) {
          reject(err);
        }
      });

      this.socket.on('data', (data: Buffer) => this._onData(data));
      this.socket.on('error', (err) => {
        if (!this.connected) {
          reject(err);
        }
        this.emit('error', err);
      });
      this.socket.on('close', () => {
        this.connected = false;
        this.emit('close');
      });
    });
  }

  close(): void {
    if (this.socket) {
      this.socket.destroy();
      this.socket = null;
    }
    this.connected = false;
  }

  private _onData(data: Buffer): void {
    this.buffer = Buffer.concat([this.buffer, data]);
    this._processBuffer();
  }

  private _processBuffer(): void {
    while (this.buffer.length >= 4) {
      const length = this.buffer.readUInt32LE(0);
      if (this.buffer.length < 4 + length) break;

      const payload = this.buffer.subarray(4, 4 + length);
      this.buffer = this.buffer.subarray(4 + length);

      const response = decode(payload) as Response;

      if (response.type === 'message') {
        this.emit('message', {
          id: response.id!,
          queue: response.queue!,
          payload: response.payload,
        } as Message);
      } else if (this.pending.length > 0) {
        const { resolve, reject } = this.pending.shift()!;
        if (response.ok === false) {
          reject(new Error(response.error || 'Unknown error'));
        } else {
          resolve(response);
        }
      }
    }
  }

  private _call(msg: Record<string, any>): Promise<Response> {
    return new Promise((resolve, reject) => {
      if (!this.socket || !this.connected) {
        reject(new Error('Not connected'));
        return;
      }

      this.pending.push({ resolve, reject });
      const packed = encode(msg);
      const header = Buffer.alloc(4);
      header.writeUInt32LE(packed.length);
      this.socket.write(Buffer.concat([header, Buffer.from(packed)]));
    });
  }

  async ping(message?: string): Promise<string> {
    const request: Record<string, any> = { cmd: 'ping' };
    if (message) request.payload = message;
    const response = await this._call(request);
    return response.pong || 'PONG';
  }

  async push(queue: string, payload: any): Promise<string> {
    const response = await this._call({ cmd: 'push', queue, payload });
    return response.id!;
  }

  async pop(queue: string, timeout = 0): Promise<Message | null> {
    const response = await this._call({ cmd: 'pop', queue, timeout });
    if (response.id) {
      return {
        id: response.id,
        queue: response.queue || queue,
        payload: response.payload,
      };
    }
    return null;
  }

  async ack(messageId: string): Promise<void> {
    await this._call({ cmd: 'ack', id: messageId });
  }

  async nack(messageId: string): Promise<void> {
    await this._call({ cmd: 'nack', id: messageId });
  }

  async length(queue: string): Promise<number> {
    const response = await this._call({ cmd: 'len', queue });
    return response.length || 0;
  }

  async delete(queue: string): Promise<boolean> {
    const response = await this._call({ cmd: 'del', queue });
    return (response.length || 0) > 0;
  }

  async subscribe(queue: string): Promise<void> {
    await this._call({ cmd: 'subscribe', queue });
    this.subscriptions.add(queue);
  }

  async unsubscribe(queue: string): Promise<void> {
    await this._call({ cmd: 'unsubscribe', queue });
    this.subscriptions.delete(queue);
  }

  onMessage(callback: (msg: Message) => void): void {
    this.on('message', callback);
  }
}

export default ZoldyQ;
