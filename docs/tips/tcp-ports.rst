=================
Handing TCP Ports
=================

There are couple of reasons you want ``lithos`` to open tcp port on behalf
of your application:

1. Running multiple instances of the application, each sharing the same port
2. Smooth upgrade of you app, where some of processes are running old version
   of software and some run new one
3. Grow and shrink number of processes without any application code to support
   that
4. Using port < 1024 and not starting process as root
5. Each process is in separate cgroup, so monitoring tools can have
   fine-grained metrics over them

.. note::

   While you could use ``SO_REUSE_PORT`` socket option for solving #1 it's not
   universally available option.

   Forking inside the application doesn't work as well as running each
   process by lithos because in the former case your memory limits apply
   to all the processes rather than being fine-grained.

Following sections describe how to configure various software stacks and
frameworks to use tcp-ports opened by lithos.

It's possible to run any software that supports `systemd socket activation`_
with :opt:`tcp-ports` of lithos. With the config similar to this:

.. _systemd socket activation: http://0pointer.de/blog/projects/socket-activation.html

.. _tp-systemd:

.. code-block:: yaml

   environ:
     LISTEN_FDS: 1   # application receives single file descriptor
     # ... more env vars ...
   tcp-ports:
     8080: # port number
       fd: 3   # SD_LISTEN_FDS_START, first fd number systemd passes
       host: 0.0.0.0
       listen-backlog: 128   # application may change this on its own
       reuse-addr: true
   # ... other process settings ...

.. _tp-asyncio:

Python3 + Asyncio
=================

For development purposes you probably have the code like this:

.. code-block:: python

    async def init(app):
        ...
        handler = app.make_handler()
        srv = await loop.create_server(handler, host, port)

To use tcp-ports you should check environment variable and pass socket
if that exists:

.. code-block:: python

    import os
    import socket

    async def init(app):
        ...
        handler = app.make_handler()
        if os.environ.get("LISTEN_FDS") == "1":
            srv = await loop.create_server(handler,
                sock=socket.fromfd(3, socket.AF_INET, socket.SOCK_STREAM))
        else:
            srv = await loop.create_server(handler, host, port)

This assumes you are configured ``environ`` and ``tcp-ports`` as
:ref:`described above<tp-systemd>`.

.. _tp-werkzeug:

Python + Werkzeug (Flask)
==========================

Werkzeug supports the functionality out of the box, just put configure the
environment:

.. code-block:: yaml

   environ:
     WERKZEUG_SERVER_FD: 3
     # ... more env vars ...
   tcp-ports:
     8080: # port number
       fd: 3  # this corresponds to WERKZEUG_SERVER_FD
       host: 0.0.0.0
       listen-backlog: 128   # default in werkzeug
       reuse-addr: true
   # ... other process settings ...

Or you can pass ``fd=3`` to ``werkzeug.serving.BaseWSGIServer``.

Another hint: **do not use processes != 1**. Better use lithos's
``instances`` to control the number of processes.


.. _tp-twisted:

Python + Twisted
================

Old code that looks like:

.. code-block:: python

    reactor.listenTCP(PORT, factory)

You need to change into something like this:

.. code-block:: python

   if os.environ.get("LISTEN_FD") == "3":
       import socket
       sock = socket.fromfd(3, socket.AF_INET, socket.SOCK_STREAM)
       sock.set_blocking(False)
       reactor.adoptStreamPort(sock.fileno(), AF_INET, factory)
       sock.close()
       os.close(3)
   else:
       reactor.listenTCP(PORT, factory)

.. _tp-golang:

Golang + net/http
=================

Previous code like this:

.. code-block:: go

    import "net/http"

    srv := &http.Server{ .. }
    if err := srv.ListenAndServe(); err != nil {
        log.Fatalf("Error listening")
    }

You should wrap into something like this:

.. code-block:: go

    import "os"
    import "net"
    import "net/http"

    srv := &http.Server{ .. }
    if os.Getenv("LISTEN_FDS") == "1" {
        listener, err := net.FileListener(os.NewFile(3, "fd 3"))
        if err != nil {
            log.Fatalf("Can't open fd 3")
        }
        if err := srv.Serve(listener); err != nil {
            log.Fatalf("Error listening on fd 3")
        }
    } else {
        if err := srv.ListenAndServe(); err != nil {
            log.Fatalf("Error listening")
        }
    }
