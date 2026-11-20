#define _DARWIN_C_SOURCE 1
#define _XOPEN_SOURCE 700
#include <dlfcn.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <execinfo.h>
#include <signal.h>
#include <ucontext.h>
#include <stdlib.h>
#include <unistd.h>
static void on_segv(int s, siginfo_t* si, void* uc){
  (void)s;
  // 信号处理器里只能调异步信号安全的函数：别用 backtrace/fprintf/dladdr
  void* pc=(void*)((ucontext_t*)uc)->uc_mcontext->__ss.__pc;
  char line[96];
  int n=snprintf(line,sizeof line,"SEGV addr=%p pc=%p\n", si->si_addr, pc);
  if(n>0) write(2,line,(size_t)n);
  _exit(9);
}

typedef struct { void* fd; const char* path; uint32_t id; } item_t;
typedef struct { item_t* items; uint32_t count; const char* user_dir; } dictcfg_t;
static item_t items[] = {
#include "items.inc"
};

static void set_verbosity(void* H){
  void* sym = dlsym(H, "wxime_get_version");
  Dl_info di; memset(&di,0,sizeof di);
  if(!dladdr(sym,&di) || !di.dli_fbase) return;
  unsigned long base = (unsigned long)di.dli_fbase;
  unsigned long addr = base + (0x10361f3f8UL - 0x100000000UL);
  *(volatile int*)addr = 9;
}
static void my_log(int l,const char*m,int n){(void)l;(void)m;(void)n;}
int main(int argc,char**argv){
  struct sigaction sa; memset(&sa,0,sizeof sa); sa.sa_sigaction=on_segv; sa.sa_flags=SA_SIGINFO; sigaction(SIGSEGV,&sa,0); sigaction(SIGBUS,&sa,0);
  void*H=dlopen(argv[1],RTLD_NOW|RTLD_LOCAL);
  if(!H){puts("dlopen fail");return 1;}
  int(*init)(const void*)=dlsym(H,"wxime_initialize");
  int(*cfg)(void*)=dlsym(H,"wxime_config_dict");
  unsigned long long(*create)(const void*)=dlsym(H,"wxime_create_session");
  static uint8_t c[512];
  *(const char**)(c+0x18)="/Library/Input Methods/WeType.app/Contents/Resources";
  *(void**)(c+0)=(void*)my_log;
  set_verbosity(H); int ri=init(c); printf("init -> %d", ri); putchar(10);
  dictcfg_t dc; dc.items=items; dc.count=sizeof(items)/sizeof(items[0]); dc.user_dir="/tmp/archtype-ref/userdata";
  set_verbosity(H); int rc=cfg(&dc); printf("config_dict -> %d", rc); putchar(10);
  static uint8_t sc[512];
  unsigned long long sid=create(sc);
  printf("create -> sid=%llu", sid); putchar(10);
  return 0;
}
