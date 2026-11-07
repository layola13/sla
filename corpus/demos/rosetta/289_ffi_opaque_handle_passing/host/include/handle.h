#ifndef DEMO_289_HANDLE_H
#define DEMO_289_HANDLE_H

typedef struct DemoHandle DemoHandle;

DemoHandle *handle_open(int id);
int handle_close(DemoHandle *handle);

#endif
