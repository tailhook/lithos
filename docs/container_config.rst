.. highlight:: yaml

.. _container_config:

=======================
Container Configuration
=======================

Container configuration is a YAML file which is usually put into
``/config/<service_name>.yaml`` into container image itself.

.. note:: Curently container configuration may be put into any folder inside
   the image, but we may fix this folder later. The arbitrary path for
   container configuration may be a security vulnerability.

The somewhat minimal configuration is looks like following:

.. code-block:: yaml

    kind: Daemon
    user_id: 1
    volumes:
      /tmp: tmpfs:size=100m
    executable: /bin/sleep
    arguments: [60]


Reference
=========

.. opt:: kind

    One of ``Daemon`` (default) and ``Command``.

    The ``Daemon`` is long-running process that is monitored by supervisor.

    The ``Command`` things are just one-off tasks, for example to initialize
    local file system data, or to check health of daemon process. The
    ``Command`` things are run by ``lithos_cmd`` utility

.. opt:: user-id

    The numeric user indentifier for the process. It must be one of the allowed
    values in lithos configuration. Usually value of ``0`` is not allowed.

.. opt:: group-id

    The numeric group indentifier for the process. It must be one of the
    allowed values in lithos configuration. Usually value of ``0`` is not
    allowed.

.. opt:: memory-limit

    The memory limit for process and it's children. This is enforced by
    cgroups. Default: nolimit. (Doesn't work yet)

.. opt:: fileno-limit

    The limit on file descriptors for process. Default ``1024``.

.. opt:: restart-timeout

    The minimum time to wait between subsequent restarts of failed processes
    in seconds.  This is to ensure that it doesn't boggles down CPU. Default
    is     ``1`` second. It's enough so that lithos itself do not hang. But
    it should be bigger for heavy-weight processes. Note: this is time between
    restarts, i.e. if process were running more than this number of seconds
    it will be restarted immediately.

.. opt:: executable

    The path to executable to run. Only absolute paths are allowed.

.. opt:: arguments

    The list of arguments for the command. Except argument zero.

.. opt:: environ

    The mapping of values that are set for process. You must set all needed
    environment variables here. The only variable that is propagated by
    default is ``TERM``. Also few special ``LITHOS_`` variables may be set.
    This means you must set all the basic ``LANG``, ``HOME`` and so on
    explicitly. This is to ensure that your environment is always the same
    regardless of where you run process.

.. opt:: workdir

    The working directory for target process. Default is ``/``. Working
    directory must be absolute.

.. opt:: resolv-conf

    Parameters of the ``/etc/resolv.conf`` file to generate. Default
    configuration is::

        resolv-conf:
            copy-from-host: true

    Which means ``resolv.conf`` from host where lithos is running is copied
    to the "state" directory of the container. More options are expected to
    be added later.

    .. warning:: To make use of it you should symlink ``ln -s
       /state/resolv.conf /etc/resolv.conf`` in the container's image. It's
       done this way so you can introspect and presumably update
       ``resolv.conf`` from the outside of container.

.. opt:: hosts-file

    Parameters of the ``/etc/hosts`` file to generate. Default
    configuration is::

        hosts-file:
            localhost: true
            public-hostname: true
            copy-from-host: false

    .. warning:: To make use of it you should symlink ``ln -s
       /state/hosts /etc/hosts`` in the container's image. It's
       done this way so you can introspect and presumably update
       ``hosts`` from the outside of container.

   Parameters:

   copy-from-host
        (default ``false``) Copy hosts file from host machine. It's false
        by default for two reasons:

        1. We want to isolate system as much as possible
        2. Some systems (like Ubuntu) like to put an address ``127.0.1.1`` on
           the public hostname, which doesn't play well with discovery
           subsystems based on hostnames (e.g. for akka_)

        Note: even if ``copy-from-host`` is ``true``, :opt:`additional-hosts`
        from sandbox config work, which may lead to duplicate or conflicting
        entries if some names are specified in both places.

   .. _akka: http://akka.io/

   localhost
        (default is true when ``copy-from-host`` is false)
        A boolean which defines whether to add
        ``127.0.0.1 localhost`` record to ``hosts``

   public-hostname
        (default is true when ``copy-from-host`` is false)
        Add to ``hosts`` file the result of ``gethostname`` system call
        along with the ip address that name resolves into.

.. opt:: uid-map, gid-map

    The list of mapping for uids(gids) in the user namespace of the container.
    If they are not specified the user namespace is not used. This setting
    allows to run processes with ``uid`` zero without the risk of being
    the ``root`` on host system.

    Here is a example of maps::

        uid-map:
        - {inside: 0, outside: 1000, count: 1}
        - {inside: 1, outside: 1, count: 1}
        gid-map:
        - {inside: 0, outside: 100, count: 1}

.. opt:: stdout-stderr-file

    This redirects both stdout and stderr to a file. The path must be absolute
    and is opened outside of the container (so depending on ``volumes`` may be
    both visible or non-visible from the container itself)

.. opt:: restart-process-only

    (default ``false``) If true when restarting process (i.e. in case
    process died or was killed), lithos restarts just the failed process.
    This means container will not be recreated, volumes will not be remounted,
    tmpfs will not be cleaned and some daemon processes may leave running.

    By default ``lithos_knot`` which is pid 1 in the container exits when
    process dies. Which means all other processes will die on ``KILL`` signal,
    and container will be removed and created again. It's a little bit slower
    but safer default. This leaves no hanging daemons, orphan files in state
    dir and tmpfs garbage.

.. opt:: volumes

    The mapping of mountpoint to volume definition. See :ref:`volumes` for more
    info

.. opt:: tcp-ports

    Binds address and provides file descriptor to the child process. All the
    children receive dup of the same file descriptor,
    so may all do ``accept()`` simultaneously. The configuration looks like::

        tcp-ports:
          7777:
            fd: 3
            host: 0.0.0.0
            listen-backlog: 128
            reuse-addr: true
            reuse-port: false

    All the fields except ``fd`` are optional.

    Programs may require to pass listening file descriptor number by some
    means (usually environment). For example to run nginx with port bound
    (so you don't need to start it as root) you need::

        tcp-ports:
          80:
            fd: 3
        environ:
          NGINX: "3;"

    To run gunicorn you may want::

        tcp-ports:
          80:
            fd: 3
        environ:
          GUNICORN_FD: "3"

    Parameters:

    *key*
      TCP port number.

      .. warning::

         * The paramters (except ``fd``) do not change after socket is
           bound even if configuration change
         * You can't bind same port with different hostnames this limitation
           may be lifted in the later versions

      Port parameter should be unique amoungst all containers. But sharing
      port works because it is useful if you are doing smooth software
      upgrade (i.e. you have few old processes running and few new processes
      running both sharing same port/file-descriptor). *Running them on single
      port is not the best practices for smooth software upgrade but that
      topic if out of scope of this documentation.*

    fd
      *Required*. File descriptor number

    host
      (default is ``0.0.0.0`` meaning all addresses) Host to bind to. It must
      be IP address, hostname is not supported.

    listen-backlog
      (default ``128``) the value to pass to the `listen()` system call. The
      value is capped by ``net.core.somaxconn``

    reuse-addr
      (default ``true``) Sets ``SO_REUSEADDR`` socket option

    reuse-port
      (default ``false``) If set to ``true`` this changes behavior of the
      lithos with respect of the socket. In default case lithos binds socket
      as quick as possible and passes to each child on start. When this set
      to ``true``, lithos creates a separate socket and calls bind for each
      process start. This has two consequences:

      * Socket is not bound when no processes started (i.e. they are failing)
      * Each process gets separate in-kernel queue of connections to accept

      This should be set to ``true`` only on very high performant servers that
      experience assymetric workload in default case.
