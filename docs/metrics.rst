=======
Metrics
=======

Lithos submits metrics via a `cantal-compatible protocol`_.

All metrics usually belong to lithos's cgroup, so for example in graphite
you can find them under ``cantal.<cluster-name>.<hostname>.lithos.groups.*``.
Or you cand find them without this prefix in
``http://hostname:22682/local/process_metrics`` without a prefix.

In the following description we skip the common prefix and only show metric
names.

Metrics of lithos master process:

* ``master.restarts`` (counter) amount of restarts of a master process.
  Usually restart equals to configuration reload via ``lithos_switch`` or any
  other way.
* ``master.sandboxes`` (gauge) number of sandboxes configured
* ``master.containers`` (gauge) number of containers (processes) conigured
* ``master.queue`` (gauge) length of the internal queue, the queue consists of
  processes to run and hanging processes to kill

Per-process metrics:

* ``processes.<sandbox_name>.<process_name>.started`` -- (counter) number of
  times process have been started
* ``processes.<sandbox_name>.<process_name>.deaths`` -- (counter) number of
  times process have exited for any reason
* ``processes.<sandbox_name>.<process_name>.failures`` -- (counter) number of
  times process have exited for failure reason, for whatever reason lithos
  thinks it was failure. Currently only processes that had been sent
  ``SIGTERM`` signal (with any exit status) or ones dead on ``SIGTERM``
  signal are considered non-failed. (*Processes exited with code 0 are still
  considered failed because daemons shoul not exit anyway*)
* ``processes.<sandbox_name>.<process_name>.running`` -- (gauge) number of
  procesess that are currently running (was started but not yet found to be
  exited)


Global metrics for all sandboxes and containers:

* ``containers.started`` -- (counter) same as for ``processes.*`` but for all
  containers
* ``containers.deaths`` -- (counter) see above
* ``containers.failures`` -- (counter) see above
* ``containers.running`` -- (gauge) see above
* ``containers.unknown`` -- (gauge) number of child processes of lithos that
  are found to be running but do not belong to any of the process groups known
  to lithos (they are being killed, and they are probably from deleted configs)

.. _cantal-compatible protocol: http://cantal.readthedocs.io/en/latest/mmap.html
