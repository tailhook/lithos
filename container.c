#include <alloca.h>
#include <unistd.h>
#include <signal.h>
#include <sched.h>
#include <unistd.h>


typedef struct {
    int namespaces;
    const char *exec_path;
    char ** const exec_args;
    char ** const exec_environ;
} CCommand;

static void _run_container(CCommand *cmd) {
    (void)execve(cmd->exec_path, cmd->exec_args, cmd->exec_environ);
    _exit(127);
}

pid_t execute_command(CCommand *cmd) {
    size_t stack_size = sysconf(_SC_PAGESIZE);
    void *stack = alloca(stack_size);

    return clone((int (*)(void*))_run_container,
        stack + stack_size,
        cmd->namespaces|SIGCHLD,
        cmd);
}

