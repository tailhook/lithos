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


.. _master-overview:

Master Config
=============

Master config contains things that are common for all containers and is
usually the same across cluster.

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
for things in case you have non-standard filesystem layout. See
:ref:`reference <master_config>` for full list of settings.


Separation of Concerns
======================

Although, it's not required, lithos is developed with the following separate
roles:

Developer
---------

Developer is the owner of the service. They needs to configure as much as
possible for their container but it's limited to:

1. They can't break into host system including:

   * remote code execution (RCE) vulnerabilities in the code
   * even if malicious party controls source code, configuration and binary
     artifacts running inside the containers

2. Service must run on any host without changes. This might include different
   filesystem layouts of the host system
   (i.e. different names/numbers of disks)

Config for developer, which we call :ref:`container config<>` comes within container itself. And usually defines command-line, environment and resource
limits:

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

This is almost it. Sometimes container need disk:

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
   (`/db` in this case). This is because otherwise the config would depend
   on exact filesystem layout on host system **and** in some cases it might
   be a vulnerability (or at least exposure of unnecessary data). Later we'll
   describe how it's mapped to the real filesystem.


