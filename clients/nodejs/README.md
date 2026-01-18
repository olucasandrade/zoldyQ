# ZoldyQ Node.js Client

Node.js/TypeScript client library for ZoldyQ message queue using the native ZQP protocol.

## Installation

```bash
npm install zoldyq
```

## Quick Start

### Basic Usage

```typescript
import { ZoldyQ } from 'zoldyq';

async function main() {
  const client = new ZoldyQ({ host: 'localhost', port: 6380 });
  await client.connect();

  // Push a message
  const msgId = await client.push('tasks', { job: 'process-image', id: 123 });
  console.log(`Pushed message: ${msgId}`);

  // Pop a message (with timeout)
  const msg = await client.pop('tasks', 30);
  if (msg) {
    console.log(`Got message: ${JSON.stringify(msg.payload)}`);
    await client.ack(msg.id);
  }

  // Check queue length
  const length = await client.length('tasks');
  console.log(`Queue length: ${length}`);

  client.close();
}

main().catch(console.error);
```

### Subscription (Server Push)

```typescript
import { ZoldyQ, Message } from 'zoldyq';

async function consumer() {
  const client = new ZoldyQ({ host: 'localhost', port: 6380 });
  await client.connect();

  // Set up message handler
  client.onMessage(async (msg: Message) => {
    console.log(`Received: ${JSON.stringify(msg.payload)}`);
    await client.ack(msg.id);
  });

  // Subscribe to queue
  await client.subscribe('tasks');
  console.log('Subscribed to tasks queue');

  // Keep the process running
  process.on('SIGINT', () => {
    client.close();
    process.exit();
  });
}

consumer().catch(console.error);
```

### With Authentication

```typescript
const client = new ZoldyQ({
  host: 'localhost',
  port: 6380,
  password: 'secret'
});
await client.connect();
```

## API Reference

### Constructor Options

```typescript
interface ZoldyQOptions {
  host?: string;    // Default: 'localhost'
  port?: number;    // Default: 6380
  password?: string;
}
```

### Methods

- `connect(): Promise<void>` - Connect to the server
- `close(): void` - Close the connection
- `ping(message?: string): Promise<string>` - Health check
- `push(queue: string, payload: any): Promise<string>` - Push a message, returns message ID
- `pop(queue: string, timeout?: number): Promise<Message | null>` - Pop a message
- `ack(messageId: string): Promise<void>` - Acknowledge a message
- `nack(messageId: string): Promise<void>` - Reject a message (will be requeued)
- `length(queue: string): Promise<number>` - Get queue length
- `delete(queue: string): Promise<boolean>` - Delete a queue
- `subscribe(queue: string): Promise<void>` - Subscribe to receive messages
- `unsubscribe(queue: string): Promise<void>` - Unsubscribe from a queue
- `onMessage(callback: (msg: Message) => void): void` - Register message handler

### Events

The client extends EventEmitter:

- `'message'` - Emitted when a subscribed message arrives
- `'error'` - Emitted on connection errors
- `'close'` - Emitted when connection closes

### Types

```typescript
interface Message {
  id: string;
  queue: string;
  payload: any;
}
```

## License

MIT
