#include <sys/prctl.h>
#include <alloca.h>
#include <unistd.h>
#include <signal.h>
#include <sched.h>
#include <unistd.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>


typedef struct {
    int namespaces;
    int user_id;
    int restore_sigmask;
    const char *logprefix;
    const char *exec_path;
    char ** const exec_args;
    char ** const exec_environ;
} CCommand;

typedef struct {
    int signo;
    pid_t pid;
    int status;
} CSignalInfo;

static void _run_container(CCommand *cmd) {
    prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0);
    if(setuid(cmd->user_id)) {
        fprintf(stderr, "%s Error setting userid %d: %m\n",
            cmd->logprefix, cmd->user_id);
        abort();
    }
    if(cmd->restore_sigmask) {
        sigset_t mask;
        sigfillset(&mask);
        sigprocmask(SIG_UNBLOCK, &mask, NULL);
    }
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

void block_all_signals() {
    sigset_t mask;
    sigfillset(&mask);
    sigprocmask(SIG_BLOCK, &mask, NULL);
}

void wait_any_signal(CSignalInfo *sig) {
    sigset_t mask;
    sigfillset(&mask);
    while(1) {
        siginfo_t native_info;
        int rc = sigwaitinfo(&mask, &native_info);
        if(rc < 0){
            if(errno == EINTR) {
                continue;
            } else {
                fprintf(stderr, "Wrong error code for sigwaitinfo: %m\n");
                abort();
            }
        }
        sig->signo = native_info.si_signo;
        sig->pid = native_info.si_pid;
        sig->status = native_info.si_status;
        return;
    }
}

