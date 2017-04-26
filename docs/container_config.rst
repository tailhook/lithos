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
    user-id: 1
    volumes:
      /tmp: !Tmpfs { size: 100m }
    executable: /bin/sleep
    arguments: [60]

.. _container_variables:

Variables
=========

Container can declare some things, that can be changed in specific
instantiation of the service, for example:

.. code-block:: yaml

    variables:
      tcp_port: !TcpPort
    kind: Daemon
    user-id: 1
    volumes:
      /tmp: !Tmpfs { size: 100m }
    executable: /bin/some_program
    arguments:
    - "--listen=localhost:@{tcp_port}"

The ``variables`` key declares variable names and types. Value for these
variables can be provided in ``variables`` in :ref:`process_config`.

There are the following types of variables:

TcpPort
    Allows a number between 1-65535 and ensures that the number matches
    port range allowed in sandbox (see :opt:`allow-tcp-ports`)
Choice
    Allows a value from a fixed set of choices
    (example: ``!Choice ["high-priority", "low-priority"]``)

All entries of ``@{variable_name}`` are substituted in the following fields:

1. :opt:`arguments`
2. The values of :opt:`environ` (not in the keys yet)
3. The key in the :opt:`tcp-ports` (i.e. port number)

The expansion in any other place does not work yet, but may be implemented
in the future. Only **declared** variables can be substituted. Trying to
substitute undeclared variables or non-existing built-in variable results
into configuration syntax error.

There are the number of builtin variables that start with ``lithos:``:

lithos:name
    Name of the process, same as inserted in ``LITHOS_NAME`` environment
    variable

lithos:config_filename
    Full path of this configuration file as visible from within container

More built-in variables may be added in the future. Built-in variables
doesn't have to be declared.


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
    cgroups, so this needs `memory` cgroup to be enabled (otherwise its no-op).
    See :opt:`cgroup-controllers` for more info.  Default: nolimit.

    You can use ``ki``, ``Mi`` and ``Gi`` units for memory accounting.
    See integer-units_.

.. opt:: cpu-shares

    The number of CPU shares for the process. Default is ``1024`` which means
    all processes get equal share. You may split them to different values
    like ``768`` for one process and ``256`` for another one.

    This is enforced by cgroups, so this needs `cpu` cgroup to be enabled
    (otherwise its no-op).  See :opt:`cgroup-controllers` for more info.

.. opt:: fileno-limit

    The limit on file descriptors for process. Default ``1024``.

.. opt:: restart-timeout

    The minimum time to wait between subsequent restarts of failed processes
    in seconds.  This is to ensure that it doesn't boggles down CPU. Default
    is     ``1`` second. It's enough so that lithos itself do not hang. But
    it should be bigger for heavy-weight processes. Note: this is time between
    restarts, i.e. if process were running more than this number of seconds
    it will be restarted immediately.

.. opt:: kill-timeout

    (default ``5`` seconds) The time to wait for application to die. If it is
    not dead by this number of seconds we kill it with ``KILL``.

    You should not rely on this timeout to be precise for multiple reasons:

    1. Unidentified children are killed with a default timeout (5 sec).
       This includes children which are being killed when their configuration
       is removed.
    2. When lithos is restarted (i.e. to reload a configuration) during
       the timeout, the timeout is reset. I.e. the process may hang more than
       this time.

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

    .. note:: Currently you may have uid-map either in a sandbox or in a
       container config, not both.

.. opt:: stdout-stderr-file

    This redirects both stdout and stderr to a file. The path is opened inside
    the container. So must reside on one of the mounted writeable
    :ref:`volumes`. Probably you want :volume:`Persistent` volume.
    While it can be on :volume:`Tmpfs` or :volume:`Statedir` the applicability
    of such thing is very limited.

    Usually log is put into the directory specified by :opt:`stdio-log-dir`.

.. opt:: interactive

    (default ``false``) Useful only for containers of kind ``Command``. If
    ``true`` lithos_cmd doesn't clobber stdin and doesn't redirect stdout and
    stderr to a log file, effectively allowing command to be used for
    interactive commands or as a part of pipeline.

    .. note:: for certain use cases, like pipelines it might be better to use
       fifo's (see ``man mkfifo``) and a ``Daemon`` instead of this one
       because daemons may be restarted on death or for software upgrade,
       while ``Command`` is not supervised by lithos.

    .. versionadded:: 0.6.3

    .. versionchanged:: ≥0.5
       Commands were always interactive

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

    More examples are in :ref:`tcp-ports-tips`

    Parameters:

    *key*
      TCP port number.

      .. warning::

         * The paramters (except ``fd``) do not change after socket is
           bound even if configuration change
         * You can't bind same port with different hostnames in a
           **single process** (previously there was a global limit for the
           single port for whole lithos master, currently this is limited
           just because ``tcp-ports`` is a mapping)

      Port parameter should be unique amongst all containers. But sharing
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

.. opt:: metadata

   (optional) Allows to add arbitrary metadata to lithos configuration file.
   Lithos does not use and does not validate this data in any way (except that
   it must be a valid YAML). The metadata can be used by other tools that
   inspect lithos configs and extract data from it. In particular, we use
   metadata for our deployment tools (to keep configuration files
   more consolidated instead of keeping then in small fragments).

.. _integer-units: http://rust-quire.readthedocs.io/en/latest/user.html#units
