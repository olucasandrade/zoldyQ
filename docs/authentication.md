# Authentication

ZoldyQ supports optional password-based authentication using the Redis `AUTH` command.

## Configuration

Set the `ZOLDYQ_PASSWORD` environment variable to enable authentication:

```bash
export ZOLDYQ_PASSWORD="your-secure-password"
./zoldyq
```

If `ZOLDYQ_PASSWORD` is not set, authentication is disabled and all commands are allowed immediately.

## Usage

When authentication is enabled, clients must authenticate before executing commands (except `PING`, `AUTH`, and `COMMAND`).

### Python

```python
import redis

r = redis.Redis(host='localhost', port=6379, password='your-secure-password')

# Or authenticate manually
r = redis.Redis(host='localhost', port=6379)
r.execute_command('AUTH', 'your-secure-password')
```

### Node.js

```javascript
const redis = require('redis');

const client = redis.createClient({
  host: 'localhost',
  port: 6379,
  password: 'your-secure-password'
});
```

### Command Line

```bash
redis-cli -p 6379 -a your-secure-password
# Or
redis-cli -p 6379
> AUTH your-secure-password
OK
```

## Error Responses

| Error | Description |
|-------|-------------|
| `NOAUTH Authentication required.` | Command rejected, client not authenticated |
| `WRONGPASS invalid password` | AUTH command failed, wrong password |

## Commands Allowed Without Auth

The following commands are allowed before authentication:

- `PING` - Connection test
- `AUTH` - Authentication
- `COMMAND` - List commands
- `QUIT` - Close connection

## Security Notes

- Passwords are transmitted in plain text unless using TLS
- Use strong, randomly generated passwords
- Consider network-level security (firewalls, VPCs)
- ZoldyQ does not support ACLs or user-based auth (single password only)
