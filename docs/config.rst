======================
Configuration Overview
======================

Lithos has 4 configs:

1. ``/etc/lithos/master.yaml`` -- global configuration for whole
   lithos daemon. Empty config should work most of the time.
   :ref:`master_config`
2. ``/etc/lithos/sandboxes/<NAME>.yaml`` -- the allowed paths and other system
   limits for every sandbox. You may think of a sandbox as a single
   application.
   :ref:`sandbox_config`
3. ``/etc/lithos/processes/<NAME>.yaml`` -- you may think of it as a list of
   pairs (image_name, num_of_processes_to_run). It's only a tiny bit
   longer than that.
   :ref:`process_config`
4. ``<IMAGE>/config/<NAME>.yaml`` -- configuration of process to run. It's
   where all the needed to run process are. It's stored inside the image (so
   updated with new image), and limited by limits in sandbox config.
   :ref:`container_config`

Four configs look superfluous, but they aren't hard. Let's see why are they
needed...


Separation of Concerns
======================

There are three roles which influence lithos containers.

.. contents::
   :local:


.. _container-overview:

Developer
---------

Developer is the owner of the service. They needs to configure as much as
possible for their container with the following limitations:

1. They can't break into host system including:

   * remote code execution (RCE) vulnerabilities in the code
   * even if malicious party controls source code, configuration and binary
     artifacts running inside the containers

2. Service must run on any host without changes. This might include different
   filesystem layouts of the host system
   (i.e. different names/numbers of disks)

Config for developer, which we call :ref:`container config<container_config>`
comes **within container itself**. And usually defines command-line,
environment and resource limits:

.. code-block:: yaml

    executable: /usr/bin/python3.6
    arguments:
    - myapp.py
    - --port=8080

    work-dir: /app
    environ:
      LANG: en_US.utf-8

    memory-limit: 100Mi
    fileno-limit: 1k

This is almost it. Sometimes container needs disk:

.. code-block:: yaml
    :emphasize-lines: 9-10

    executable: /usr/bin/python3.6
    arguments:
    - myapp.py
    - --port=8080

    work-dir: /app
    environ:
      LANG: en_US.utf-8
    volumes:
      /var/lib/sqlite: !Persistent /db

    memory-limit: 100Mi
    fileno-limit: 1k

Note the following things:

1. It doesn't define where filesystem root is because config itself lies in
   the filesystem root.
2. Volumes don't specify path in the host filesystem, it's a virtual path
   (``/db`` in this case). This is because otherwise the config would depend
   on exact filesystem layout on host system **and** in some cases it might
   be a vulnerability (or at least exposure of unnecessary data). Later we'll
   describe how it's mapped to the real filesystem.

Container operates inside a **sandbox** defined by platform maintainers.

.. _sandbox-overview:

Platform Maintainer
-------------------

Platform maintainers define how containers are run. They define :ref:`sandbox
config<sandbox_config>` and :ref:`master config<master_config>`.

Former defines sandbox for a specific application.
Let's see an example *(don't use it in production, see below)*:

.. code-block:: yaml

    image-dir: /opt/app1-images
    allow-users: [100000-165535]
    allow-groups: [100000-165535]
    default-user: 100000
    default-group: 100000

This says that images of this application are in ``/opt/app1-images`` and
it's allowed to use user-ids in the range ``100000-165535``.

User Namespaces
```````````````

First thing to configure here is to make a user namespace per application:

.. code-block:: yaml

    image-dir: /opt/app1-images

    uid-map:
    - { inside: 0, outside: 10002, count: 2 }
    gid-map:
    - { inside: 0, outside: 10002, count: 2 }

    allow-users: [1]
    allow-groups: [1]
    default-user: 1
    default-group: 1

Note the following things:

1. We introduced uid/gid map. this means that two users starting with user id
   ``10002`` in the host system will be two users ``0,1`` in the container.
2. Allowed and default users are set relative to the container ids not host
   system ones
3. We allow only single user id and group id in the container. And this is
   number ``1`` (i.e. first non-root user)
4. This scheme works for 99% applications. But in case you need containers in
   containers or some other specific scenario you can enlarge uid-map and
   allowed groups as much as OS allows.

The id ``10002`` is arbitrary. You can use any one. For security and monitoring
purposes you should keep separate user ids for each app. Whether they are
same across the cluster or allocated on each node is irrelevant unless you
have shared filesystem between machines. *Keeping them same uids across
cluster is still recommended for easier monitoring and debugging*.

You can allow uid ``0`` too. When using uid name spaces it **should not**
cause any elevated privileges. But this allows creating mountpoints, spawning
other namespaces and do lots of things which creates larger vector of attack.
This has caused vulnerabilities due to kernel bugs in the past.


Filesystem
``````````

As you have already seen, sandbox config defines a place with container
base directories:

.. code-block:: yaml

   image-dir: /opt/app1-images
   image-dir-levels: 1   # default value

In this config, directories named like this ``/opt/app1-images/some-name1``
serve as the root directory for containers(we'll show later how to find out
which specific directory is used now). They are mounted **readonly**. With this
config:

.. code-block:: yaml

   image-dir: /opt/app2-images
   image-dir-levels: 2

Images are located in ``/opt/app2-images/service1/version1``. I.e. two
directory components below the image dir. Arbitrary :opt:`image-dir-levels`
can be used. Only fixed number of components supported for each specific
sandbox, though.

Extra directories can be specified as follows:

.. code-block:: yaml

    readonly-paths:
      /timezones: /usr/share/timezones

    writable-paths:
      /db: /var/lib/app1-database

There are virtual paths on the left. These can be mounted by referencing them
in :ref:`container config <container_config>`:

.. code-block:: yaml

    executable: /usr/bin/python3.6
    arguments:
    - myapp.py
    - --port=8080
    volumes:
      /etc/timezones: !Readonly /timezones
      /var/lib/sqlite: !Persistent /db

This allows platform maintainers to move directories around in the host
system and map different directories on different systems without ever
interfering the container.


Network
```````

Sandbox also contains network configuration. By default all containers have
host network (i.e. they operate in the same network namespace, just like
non-containerized processes).

There is also support for bridged network:

.. code-block:: yaml

    bridged-network:
      bridge: br0
      network: 10.64.0.0/16
      default-gateway: 10.64.255.254
      after-setup-command: [/usr/bin/arping, -U, -c1, '@{container_ip}']

This enables network isolation for containers.  Every container in the sandbox
have its own network config with separate IP address (see below which one) but
all of them derive their configuration from the sandbox config.

Different sandboxes may have the same or different bridged network configs.


See :ref:`reference <sandbox_config>` for more info.

.. _master-overview:

Master Config
`````````````

Along with sandbox configs, master config is also a part of the "platform
maintainer" zone of responsibility. It contains things that are common for all
containers and is usually the same across cluster.

You may run with **empty** config. But most commonly it's expected to contain
cgroup controllers for lithos to manage:

.. code-block:: yaml

    cgroup-controllers: [name,cpu,memory]

This is needed for lithos to correctly support memory limits and CPU quotes.

You might also want to nullify :opt:`config-log-dir` if you don't use
``lithos_clean``:

.. code-block:: yaml

    cgroup-controllers: [name,cpu,memory]
    config-log-dir: null

Usually, you don't need to set anything else. There are various directories
to configure in case you have non-standard filesystem layout. See
:ref:`reference <master_config>` for full list of settings.


Orchestration System
--------------------

The last part of configuration is thing that ties sandboxes, images and
container configs together. We call it :ref:`process config <process_config>`.

The general idea is that this config is created by an orchestration system.
I.e. system that decides where, which version and how many processes to run.
This can be some real system like verwalter_ or just an ansible/chef/salt/bash
script that writes required configs.

Basically it looks like:

.. code-block:: yaml

    web-worker:
        kind: Daemon
        image: web-wrk/d7399260
        config: "/config/web-worker.yaml"
        instances: 2

    background-worker:
        kind: Daemon
        image: task-queue/d7399260
        config: "/config/task-queue.yaml"
        instances: 3

Here we run two kinds of services "web worker" with 2 instances
(equal processes/containers) and "background worker" with 3 instances.

The ``image`` is a directory path relative to :opt:`image-dir`. Path must
contain the number of path components specified in :opt:`image-dir-levels`.
It is also expected that the diretory is immutable, so each new version
of container is run from a different directory and directory path contains
some notion of the container version.

``config`` is the path inside the container. There is no limit on how many
configs might be in the same container. Not all of them might be running at
any moment in time.

There are few other things that can be configured in this config.
If you're using bridged networking, you need to specify IP address for each
container:

.. code-block:: yaml

    web-worker:
        kind: Daemon
        image: web-wrk/d7399260
        config: "/config/web-worker.yaml"
        instances: 2
        ip_addresses:
        - 10.64.0.10
        - 10.64.0.11

And sometimes containers allow to customize their config with
:ref:`variables <container_variables>`:


.. code-block:: yaml

    background-worker:
      kind: Daemon
      image: task-queue/d7399260
      config: "/config/task-queue.yaml"
      instances: 3
      variables:
        queue_name: "main-queue"

See :ref:`reference <process_config>` for more info.

.. _verwalter: https://verwalter.readthedocs.io/en/latest/
