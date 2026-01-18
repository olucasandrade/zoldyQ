# ZoldyQ Go Client

Go client library for ZoldyQ message queue using the native ZQP protocol.

## Installation

```bash
go get github.com/zoldyq/zoldyq-go/zoldyq
```

## Quick Start

### Basic Usage

```go
package main

import (
    "fmt"
    "log"

    "github.com/zoldyq/zoldyq-go/zoldyq"
)

func main() {
    client := zoldyq.NewClient(zoldyq.Options{
        Host: "localhost",
        Port: 6380,
    })

    if err := client.ConnectAddr("localhost:6380"); err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    // Push a message
    msgID, err := client.Push("tasks", map[string]interface{}{
        "job": "process-image",
        "id":  123,
    })
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Pushed message: %s\n", msgID)

    // Pop a message (with 30 second timeout)
    msg, err := client.Pop("tasks", 30)
    if err != nil {
        log.Fatal(err)
    }
    if msg != nil {
        fmt.Printf("Got message: %v\n", msg.Payload)
        if err := client.Ack(msg.ID); err != nil {
            log.Fatal(err)
        }
    }

    // Check queue length
    length, err := client.Length("tasks")
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Queue length: %d\n", length)
}
```

### Subscription (Server Push)

```go
package main

import (
    "fmt"
    "log"
    "os"
    "os/signal"
    "syscall"

    "github.com/zoldyq/zoldyq-go/zoldyq"
)

func main() {
    client := zoldyq.NewClient(zoldyq.Options{
        Host: "localhost",
        Port: 6380,
    })

    if err := client.ConnectAddr("localhost:6380"); err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    // Subscribe to queue
    msgChan, err := client.Subscribe("tasks")
    if err != nil {
        log.Fatal(err)
    }

    // Handle shutdown
    sigChan := make(chan os.Signal, 1)
    signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

    fmt.Println("Subscribed to tasks queue. Press Ctrl+C to exit.")

    for {
        select {
        case msg := <-msgChan:
            fmt.Printf("Received: %v\n", msg.Payload)
            if err := client.Ack(msg.ID); err != nil {
                log.Printf("Ack failed: %v", err)
            }
        case <-sigChan:
            fmt.Println("Shutting down...")
            return
        }
    }
}
```

### With Authentication

```go
client := zoldyq.NewClient(zoldyq.Options{
    Host:     "localhost",
    Port:     6380,
    Password: "secret",
})
```

## API Reference

### Options

```go
type Options struct {
    Host     string        // Default: "localhost"
    Port     int           // Default: 6380
    Password string        // Optional authentication
    Timeout  time.Duration // Connection timeout, default: 30s
}
```

### Client Methods

- `NewClient(opts Options) *Client` - Create a new client
- `Connect() error` - Connect using configured host:port
- `ConnectAddr(addr string) error` - Connect to specific address
- `Close() error` - Close the connection
- `Ping(message string) (string, error)` - Health check
- `Push(queue string, payload interface{}) (string, error)` - Push a message
- `Pop(queue string, timeout int) (*Message, error)` - Pop a message
- `Ack(messageID string) error` - Acknowledge a message
- `Nack(messageID string) error` - Reject a message
- `Length(queue string) (int64, error)` - Get queue length
- `Delete(queue string) (bool, error)` - Delete a queue
- `Subscribe(queue string) (<-chan Message, error)` - Subscribe to messages
- `Unsubscribe(queue string) error` - Unsubscribe from a queue

### Types

```go
type Message struct {
    ID      string
    Queue   string
    Payload interface{}
}
```

## License

MIT
