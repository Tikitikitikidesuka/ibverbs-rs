# Internet Access from lbdaq Server

To work directly in one of the lbdaq development machines with internet access,
a reverse SOCKS proxy can be used to route traffic through the connecting machine.

## Setting SOCKS proxy up

### On your client machine

```bash
ssh -R PORT USERNAME@REMOTE_SERVER_IP
```

For this project, for example, the rusteb01.lbdaq.cern.ch server is used, so:

```bash
ssh -r 2222 USERNAME@rusteb01.lbdaq.cern.ch
```

Substituting your online username on `USERNAME`.

### On the remote server

Setup the proxy environment variables on your session:

```bash
export http_proxy=socks5://localhost:PORT
export https_proxy=socks5://localhost:PORT
export HTTP_PROXY=socks5://localhost:PORT
export HTTPS_PROXY=socks5://localhost:PORT
```

Now you should have access to the internet as long as the SOCKS server remains open on your machine.