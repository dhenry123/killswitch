# Killswitch Server

A secure, two-stage killswitch server written in Rust that provides controlled remote execution of system commands through a double-authentication mechanism.

## Overview

The Killswitch Server acts as a safety mechanism that allows authorized remote triggering of system operations through a two-secret authentication process. It's designed for scenarios where you need a reliable way to execute critical operations remotely while maintaining security through multiple verification steps.

## How It Works

### Two-Stage Authentication

1. **First Secret Verification**

   - Client sends a request containing the primary secret
   - Server validates the first secret and generates a unique, random second secret
   - Server returns the second secret to the client and stores it temporarily

2. **Second Secret Verification**
   - Client sends a subsequent request containing the generated second secret
   - Server validates the second secret and executes the configured operations
   - The second secret is immediately invalidated after use

### Operational Flow

```
Client Request (1st secret)
    → Server validates → Generates 2nd secret → Returns to client
    → Client stores 2nd secret

Client Request (2nd secret)
    → Server validates → Executes kill hook immediately
    → Schedules restore hook after 5 minutes
    → Invalidates used 2nd secret
```

## Features

- **Dual Authentication**: Requires two separate secrets for operation
- **Ephemeral Secrets**: Second secrets are single-use and time-limited
- **Hook System**: Executes customizable shell scripts for operations
- **Automatic Restoration**: Built-in delayed restore capability
- **Thread-Safe**: Handles multiple concurrent connections safely
- **Configurable**: Command-line configuration for all parameters

## Usage

### Command Line Arguments

```bash
./killswitch \
    --port 8000 \               # Listening port (default: 8000)
    --first-secret-file "/etc/killswitchpasswd" \  # Primary secret file
    --restore-delay 60 \  # Delay between kill and restore in secondes
    --kill-hook "/path/to/kill.sh" \    # Script to execute immediately
    --restore-hook "/path/to/restore.sh" # Script to execute after 5 minutes
```

### Example Configuration

```bash
./killswitch \
    --port 8080 \
    --first-secret-file "/etc/killswitchpasswd" \
    --restore-delay 60 \
    --kill-hook "/opt/scripts/emergency_shutdown.sh" \
    --restore-hook "/opt/scripts/restore_services.sh"
```

### API Endpoints

The server responds to HTTP requests on the configured port:

- **First Secret Request**: Any request containing the first secret in its body
  - Response: Returns a unique second secret
- **Second Secret Request**: Any request containing a valid second secret
  - Response: Executes hooks and returns success message

## Security Considerations

- **Secret Generation**: Second secrets are 12-character random strings (a-zA-Z)
- **Single Use**: Each second secret can only be used once
- **No Persistence**: Secrets are stored in memory only and lost on server restart
- **Immediate Invalidation**: Used secrets are immediately removed
- **Hook Isolation**: Shell scripts execute with system user permissions

## Use Cases

- **Emergency Shutdown Systems**: Gracefully terminate services in emergency situations
- **Security Incidents**: Isolate systems during security breaches
- **Maintenance Operations**: Coordinate distributed system maintenance
- **Disaster Recovery**: Trigger recovery procedures remotely
- **Safety Systems**: Industrial or IoT safety mechanisms

## Hook Specifications

### Kill Hook

- Executed immediately upon second secret verification
- Should contain commands to stop services, isolate systems, or trigger safety measures

### Restore Hook

- Executed automatically 5 minutes after kill hook
- Should contain commands to restore normal operations
- Can be used to re-enable services or reverse kill operations

## Example Hook Scripts

### kill.sh

```bash
#!/bin/bash
logger -t killswitch "Activating emergency shutdown at $(date)"
systemctl stop apache2
systemctl stop postgresql
iptables -A INPUT -p tcp --dport 80 -j DROP
```

### restore.sh

```bash
#!/bin/bash
logger -t killswitch "Restoring services at $(date)"
iptables -D INPUT -p tcp --dport 80 -j DROP
systemctl start postgresql
systemctl start apache2
```

## Requirements

- Rust 1.89+
- Linux/Unix system (for shell hook execution)
- Appropriate permissions to execute hook scripts

## Building

```bash
cargo build --release
```

## Logging

The server provides detailed logging with timestamps for:

- Server startup and shutdown
- Secret validation attempts
- Hook execution results
- Error conditions

## Limitations

- Secrets are not persisted across server restarts
- HTTP-only communication (consider HTTPS reverse proxy for production)
- Basic authentication through string matching in request body
- 5-minute restore delay is fixed in code

## Security Recommendations

1. Run behind HTTPS reverse proxy for encrypted communication or VPN
2. Use strong, randomly generated first secrets
3. Regularly rotate first secrets in production environments
4. Ensure hook scripts have minimal required permissions
5. Monitor server logs for unauthorized access attempts
6. Consider network-level isolation for the killswitch server

## Installation

- Create hook scripts:
  - /etc/killswitchpasswd (chmod 700)
  - /usr/local/share/killswitch/hooks/kill_example.sh
  - /usr/local/share/killswitch/hooks/restore_example.sh

run as "root":

```bash
set -eu
SERVICE_NAME="killswith"
cargo build --release
cp target/release/killswith /usr/local/bin/.
chmod +x /usr/local/bin/${SERVICE_NAME}
USER_NAME="root" # but depends of operation executed by hooks
cat > "/etc/systemd/system/${SERVICE_NAME}.service" << EOF
[Unit]
Description=Killswitch Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=$USER_NAME
Group=$USER_NAME
WorkingDirectory=$CONFIG_DIR
ExecStart=/usr/local/bin/${SERVICE_NAME} \\
    --port 8080 \\
    --first-secret-file "/etc/killswitchpasswd" \\
    --kill-hook "/usr/local/share/killswitch/hooks/kill_example.sh" \\
    --restore-hook "/usr/local/share/killswitch/hooks/restore_example.sh" \\
    --restore-delay 300
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
# Reload systemd, enable and start service
echo "Reloading systemd daemon..."
systemctl daemon-reload

echo "Enabling $SERVICE_NAME service to start at boot..."
systemctl enable "$SERVICE_NAME.service"

echo "Starting $SERVICE_NAME service..."
systemctl start "$SERVICE_NAME.service"

```

## Author and licence

@author: dhenry for mytinydc.com

Licence: MIT
