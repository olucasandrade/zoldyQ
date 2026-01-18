package zoldyq

import (
	"encoding/binary"
	"errors"
	"fmt"
	"net"
	"sync"
	"time"

	"github.com/vmihailenco/msgpack/v5"
)

type Message struct {
	ID      string      `msgpack:"id"`
	Queue   string      `msgpack:"queue"`
	Payload interface{} `msgpack:"payload"`
}

type request struct {
	Cmd      string      `msgpack:"cmd"`
	Queue    string      `msgpack:"queue,omitempty"`
	Payload  interface{} `msgpack:"payload,omitempty"`
	Timeout  int         `msgpack:"timeout,omitempty"`
	ID       string      `msgpack:"id,omitempty"`
	Password string      `msgpack:"password,omitempty"`
}

type response struct {
	OK      bool        `msgpack:"ok"`
	Error   string      `msgpack:"error,omitempty"`
	ID      string      `msgpack:"id,omitempty"`
	Payload interface{} `msgpack:"payload,omitempty"`
	Queue   string      `msgpack:"queue,omitempty"`
	Length  int64       `msgpack:"length,omitempty"`
	Pong    string      `msgpack:"pong,omitempty"`
	Type    string      `msgpack:"type,omitempty"`
}

type Options struct {
	Host     string
	Port     int
	Password string
	Timeout  time.Duration
}

type Client struct {
	conn     net.Conn
	mu       sync.Mutex
	opts     Options
	msgChan  chan Message
	closeCh  chan struct{}
	closed   bool
}

func NewClient(opts Options) *Client {
	if opts.Host == "" {
		opts.Host = "localhost"
	}
	if opts.Port == 0 {
		opts.Port = 6380
	}
	if opts.Timeout == 0 {
		opts.Timeout = 30 * time.Second
	}

	return &Client{
		opts:    opts,
		msgChan: make(chan Message, 100),
		closeCh: make(chan struct{}),
	}
}

func (c *Client) Connect() error {
	addr := fmt.Sprintf("%s:%d", c.opts.Host, c.opts.Port)
	conn, err := net.DialTimeout("tcp", addr, c.opts.Timeout)
	if err != nil {
		return err
	}
	c.conn = conn

	if c.opts.Password != "" {
		_, err := c.call(request{Cmd: "auth", Password: c.opts.Password})
		if err != nil {
			c.conn.Close()
			return err
		}
	}

	return nil
}

func (c *Client) ConnectAddr(addr string) error {
	conn, err := net.DialTimeout("tcp", addr, c.opts.Timeout)
	if err != nil {
		return err
	}
	c.conn = conn

	if c.opts.Password != "" {
		_, err := c.call(request{Cmd: "auth", Password: c.opts.Password})
		if err != nil {
			c.conn.Close()
			return err
		}
	}

	return nil
}

func (c *Client) Close() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.closed {
		return nil
	}
	c.closed = true
	close(c.closeCh)

	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

func (c *Client) send(req request) error {
	data, err := msgpack.Marshal(req)
	if err != nil {
		return err
	}

	header := make([]byte, 4)
	binary.LittleEndian.PutUint32(header, uint32(len(data)))

	c.mu.Lock()
	defer c.mu.Unlock()

	if _, err := c.conn.Write(header); err != nil {
		return err
	}
	if _, err := c.conn.Write(data); err != nil {
		return err
	}

	return nil
}

func (c *Client) recv() (*response, error) {
	header := make([]byte, 4)
	if _, err := c.conn.Read(header); err != nil {
		return nil, err
	}

	length := binary.LittleEndian.Uint32(header)
	data := make([]byte, length)

	totalRead := 0
	for totalRead < int(length) {
		n, err := c.conn.Read(data[totalRead:])
		if err != nil {
			return nil, err
		}
		totalRead += n
	}

	var resp response
	if err := msgpack.Unmarshal(data, &resp); err != nil {
		return nil, err
	}

	return &resp, nil
}

func (c *Client) call(req request) (*response, error) {
	if err := c.send(req); err != nil {
		return nil, err
	}
	return c.recv()
}

func (c *Client) Ping(message string) (string, error) {
	req := request{Cmd: "ping"}
	if message != "" {
		req.Payload = message
	}

	resp, err := c.call(req)
	if err != nil {
		return "", err
	}
	if !resp.OK {
		return "", errors.New(resp.Error)
	}

	if resp.Pong != "" {
		return resp.Pong, nil
	}
	return "PONG", nil
}

func (c *Client) Push(queue string, payload interface{}) (string, error) {
	resp, err := c.call(request{
		Cmd:     "push",
		Queue:   queue,
		Payload: payload,
	})
	if err != nil {
		return "", err
	}
	if !resp.OK {
		return "", errors.New(resp.Error)
	}

	return resp.ID, nil
}

func (c *Client) Pop(queue string, timeout int) (*Message, error) {
	resp, err := c.call(request{
		Cmd:     "pop",
		Queue:   queue,
		Timeout: timeout,
	})
	if err != nil {
		return nil, err
	}
	if !resp.OK {
		return nil, errors.New(resp.Error)
	}

	if resp.ID == "" {
		return nil, nil
	}

	return &Message{
		ID:      resp.ID,
		Queue:   resp.Queue,
		Payload: resp.Payload,
	}, nil
}

func (c *Client) Ack(messageID string) error {
	resp, err := c.call(request{
		Cmd: "ack",
		ID:  messageID,
	})
	if err != nil {
		return err
	}
	if !resp.OK {
		return errors.New(resp.Error)
	}

	return nil
}

func (c *Client) Nack(messageID string) error {
	resp, err := c.call(request{
		Cmd: "nack",
		ID:  messageID,
	})
	if err != nil {
		return err
	}
	if !resp.OK {
		return errors.New(resp.Error)
	}

	return nil
}

func (c *Client) Length(queue string) (int64, error) {
	resp, err := c.call(request{
		Cmd:   "len",
		Queue: queue,
	})
	if err != nil {
		return 0, err
	}
	if !resp.OK {
		return 0, errors.New(resp.Error)
	}

	return resp.Length, nil
}

func (c *Client) Delete(queue string) (bool, error) {
	resp, err := c.call(request{
		Cmd:   "del",
		Queue: queue,
	})
	if err != nil {
		return false, err
	}
	if !resp.OK {
		return false, errors.New(resp.Error)
	}

	return resp.Length > 0, nil
}

func (c *Client) Subscribe(queue string) (<-chan Message, error) {
	resp, err := c.call(request{
		Cmd:   "subscribe",
		Queue: queue,
	})
	if err != nil {
		return nil, err
	}
	if !resp.OK {
		return nil, errors.New(resp.Error)
	}

	go c.subscriptionLoop()

	return c.msgChan, nil
}

func (c *Client) subscriptionLoop() {
	for {
		select {
		case <-c.closeCh:
			return
		default:
			resp, err := c.recv()
			if err != nil {
				return
			}

			if resp.Type == "message" {
				msg := Message{
					ID:      resp.ID,
					Queue:   resp.Queue,
					Payload: resp.Payload,
				}

				select {
				case c.msgChan <- msg:
				case <-c.closeCh:
					return
				}
			}
		}
	}
}

func (c *Client) Unsubscribe(queue string) error {
	resp, err := c.call(request{
		Cmd:   "unsubscribe",
		Queue: queue,
	})
	if err != nil {
		return err
	}
	if !resp.OK {
		return errors.New(resp.Error)
	}

	return nil
}
