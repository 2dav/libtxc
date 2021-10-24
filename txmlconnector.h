typedef unsigned char BYTE;

BYTE* Initialize	(const BYTE*, int );
BYTE* InitializeEx	(const BYTE*);
BYTE* SetLogLevel	(int);
BYTE* SendCommand	(BYTE*);
_Bool  SetCallback	((*)(BYTE*));
_Bool  SetCallbackEx ((*)(BYTE*, void*), void*);
_Bool  FreeMemory	(BYTE*);
BYTE* UnInitialize	();
/* __stdcall GetServiceInfo(); */
