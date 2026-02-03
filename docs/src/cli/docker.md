# Docker Image

As a lower-cost alternative to the local installation, you may install and use the CLI app
from the [GitHub Container registry](https://github.com/slowli/term-transcript/pkgs/container/term-transcript).
To run the app in a Docker container, use a command like

```bash
docker run -i --rm --env COLOR=always \
  ghcr.io/slowli/term-transcript:master \
  print - < examples/rainbow.svg
```

Here, the `COLOR` env variable sets the coloring preference for the output,
and the `-` arg for the `print` subcommand instructs reading from stdin.

Running `exec` and `test` subcommands from a Docker container is more tricky
since normally this would require taking the entire environment for the executed commands
into the container. In order to avoid this, you can establish a bidirectional channel
with the host using [`nc`](https://linux.die.net/man/1/nc), which is pre-installed
in the Docker image:

```bash
docker run --rm -v /tmp/shell.sock:/tmp/shell.sock \
  ghcr.io/slowli/term-transcript:master \
  exec --shell nc --echoing --args=-U --args=/tmp/shell.sock 'ls -al'
```

Here, the complete shell command connects `nc` to the Unix domain socket
at `/tmp/shell.sock`, which is mounted to the container using the `-v` option.

On the host side, connecting the `bash` shell to the socket could look like this:

```bash
mkfifo /tmp/shell.fifo
cat /tmp/shell.fifo | bash -i 2>&1 | nc -lU /tmp/shell.sock > /tmp/shell.fifo &
```

Here, `/tmp/shell.fifo` is a FIFO pipe used to exchange data between `nc` and `bash`.
The drawback of this approach is that the shell executable
would not run in a (pseudo-)terminal and thus could look differently (no coloring etc.).
To connect a shell in a pseudo-terminal, you can use [`socat`](http://www.dest-unreach.org/socat/doc/socat.html),
changing the host command as follows:

```bash
socat UNIX-LISTEN:/tmp/shell.sock,fork EXEC:"bash -i",pty,setsid,ctty,stderr &
```

TCP sockets can be used instead of Unix sockets, but are not recommended
if Unix sockets are available since they are less secure. Indeed, care should be taken
that the host "server" is not bound to a publicly accessible IP address, which
would create a remote execution backdoor to the host system. As usual, caveats apply;
e.g., one can spawn the shell in another Docker container connecting it and the `term-transcript`
container in a single Docker network. In this case, TCP sockets are secure and arguably
easier to use given Docker built-in DNS resolution machinery.
