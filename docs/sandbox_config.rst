.. _sandbox_config:

==============
Sandbox Config
==============


This config resides in ``/etc/lithos/sandboxes/NAME.yaml`` (by default).
Where ``NAME`` is the name of a sandbox.

The configuration file contains security and resource limits for the container.
Including:

* A directory where image resides
* Set of directories that are mounted inside the container (i.e. all writable
  directories for the container, the ``/tmp``...)
* ulimit settings
* cgroup limits

Reference
=========


.. opt:: config-file

   The path for the :ref:`processes config<process_config>`.  In most cases
   should be left unset.  Default is ``null`` which is results into
   ``/etc/lithos/processes/NAME.yaml`` with all other settings being defaults.

.. opt:: image-dir

   Directory where application images are. Every subdir of the ``image-dir``
   may be mounted as a root file system in the container. **Required**.

.. opt:: log-file

   The file name where to put **supervisor** log of the container. Default is
   ``/var/log/lithos/SANDBOX_NAME.yaml``.

.. opt:: log-level

   (default ``warn``). The logging level of the supervisor.

.. opt:: readonly-paths

   The mapping of ``virtual_directory: host_system_directory`` of folders which
   are visible for the container in read-only mode. (Note currently if you
   have submounts in the source directory, thay may be available as writeable).
   See :ref:`Volumes` for more details.

.. opt:: writable-paths

   The mapping of ``virtual_directory: host_system_directory`` of folders which
   are visible for the container in writable mode.
   See :ref:`Volumes` for more details.

.. opt:: allow-users

   List of ranges of user ids which can be used by container. For containers
   which do not have user namespaces enabled it's just a limit for ``user-id``
   setting. For user namespaces it limits all the user ids available to
   the namespace.

   Example::

    allow-users: [1, 99, 1000-2000]

.. opt:: allow-groups

   List of ranges of group ids for the container.
   Works similarly to :opt:`allow-users`.

.. opt:: allow-tcp-ports

   List of ranges of allowed TCP ports for container. This is currently not
   enforced in any way except:

   1. Ports < 1024 are restricted by OS for non-root (but may be allowed here)
   2. It restricts :opt:`bind-port` setting in container config

   .. note:: if you have overlapping TCP port for different sandboxes, only
      single file descriptor will be used for each port. The config for
      opening port will be used arbitrary from single config amonst all users,
      which have obvious security implications.

.. opt:: additional-hosts

   Mapping of ``hostname: ip`` for names that will be added to ``/etc/hosts``
   file. This is occasinally used for cheap but static service discovery.




