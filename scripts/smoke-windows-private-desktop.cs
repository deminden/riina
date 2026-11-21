using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class RiinaPrivateDesktop
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct StartupInfo
    {
        public UInt32 Size;
        public string Reserved;
        public string Desktop;
        public string Title;
        public UInt32 X, Y, XSize, YSize, XCells, YCells, Fill, Flags;
        public UInt16 ShowWindow, ReservedBytes;
        public IntPtr ReservedPointer, StandardInput, StandardOutput, StandardError;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct ProcessInfo
    {
        public IntPtr Process, Thread;
        public UInt32 ProcessId, ThreadId;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct BasicLimits
    {
        public Int64 PerProcessTimeLimit, PerJobTimeLimit;
        public UInt32 Flags;
        public UIntPtr MinimumWorkingSetSize, MaximumWorkingSetSize;
        public UInt32 ActiveProcessLimit;
        public UIntPtr Affinity;
        public UInt32 PriorityClass, SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct IoCounters
    {
        public UInt64 ReadOperations, WriteOperations, OtherOperations;
        public UInt64 ReadBytes, WrittenBytes, OtherBytes;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct ExtendedLimits
    {
        public BasicLimits Basic;
        public IoCounters Io;
        public UIntPtr ProcessMemoryLimit, JobMemoryLimit, PeakProcessMemory, PeakJobMemory;
    }

    private const UInt32 DesktopAccess = 0x0043;
    private const UInt32 DesktopReadObjects = 0x0001;
    private const UInt32 CreateSuspended = 0x00000004;
    private const UInt32 CreateNoWindow = 0x08000000;
    private const UInt32 KillOnJobClose = 0x00002000;
    private const UInt32 WaitTimeout = 0x00000102;
    private const int UserObjectName = 2;
    private const int ExtendedLimitInformation = 9;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateJobObject(IntPtr attributes, string name);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(
        IntPtr job, int kind, ref ExtendedLimits limits, UInt32 length);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateJobObject(IntPtr job, UInt32 exitCode);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern UInt32 ResumeThread(IntPtr thread);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateDesktop(
        string name, string device, IntPtr mode, UInt32 flags, UInt32 access, IntPtr security);
    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll", SetLastError = true)]
    private static extern IntPtr OpenInputDesktop(UInt32 flags, bool inherit, UInt32 access);
    [DllImport("user32.dll", SetLastError = true)]
    private static extern IntPtr GetThreadDesktop(UInt32 thread);
    [DllImport("kernel32.dll")]
    private static extern UInt32 GetCurrentThreadId();
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool GetUserObjectInformation(
        IntPtr handle, int index, StringBuilder value, UInt32 size, out UInt32 needed);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CreateProcess(
        string application, StringBuilder command, IntPtr processAttributes,
        IntPtr threadAttributes, bool inherit, UInt32 flags, IntPtr environment,
        string directory, ref StartupInfo startup, out ProcessInfo process);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern UInt32 WaitForSingleObject(IntPtr handle, UInt32 milliseconds);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetExitCodeProcess(IntPtr process, out UInt32 status);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(IntPtr handle, UInt32 status);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    private static string NameOf(IntPtr desktop)
    {
        if (desktop == IntPtr.Zero)
            throw new Win32Exception();
        StringBuilder name = new StringBuilder(256);
        UInt32 required;
        if (!GetUserObjectInformation(desktop, UserObjectName, name, (UInt32) name.Capacity * 2, out required))
            throw new Win32Exception();
        return name.ToString();
    }

    public static string CurrentThreadDesktop()
    {
        return NameOf(GetThreadDesktop(GetCurrentThreadId()));
    }

    public static string CurrentInputDesktop()
    {
        IntPtr desktop = OpenInputDesktop(0, false, DesktopReadObjects);
        try { return NameOf(desktop); }
        finally { if (desktop != IntPtr.Zero) CloseDesktop(desktop); }
    }

    public static UInt32 Run(string name, string application, string[] arguments, string directory, UInt32 timeout)
    {
        IntPtr desktop = IntPtr.Zero;
        int failure = 0;
        // Desktop creation associates it with the calling thread.
        // Use a short-lived thread so closing the desktop is possible.
        Thread factory = new Thread(() => {
            desktop = CreateDesktop(name, null, IntPtr.Zero, 0, DesktopAccess, IntPtr.Zero);
            if (desktop == IntPtr.Zero)
                failure = Marshal.GetLastWin32Error();
        });
        factory.Start();
        factory.Join();
        if (desktop == IntPtr.Zero)
            throw new Win32Exception(failure);

        try {
            return RunOnDesktop(name, application, arguments, directory, timeout);
        } finally {
            CloseDesktop(desktop);
        }
    }

    private static UInt32 RunOnDesktop(string name, string application, string[] arguments, string directory, UInt32 timeout)
    {
        var job = CreateJobObject(IntPtr.Zero, null);
        if (job == IntPtr.Zero)
            throw new Win32Exception();
        try {
            var limits = new ExtendedLimits { Basic = new BasicLimits { Flags = KillOnJobClose } };
            if (!SetInformationJobObject(job, ExtendedLimitInformation, ref limits, (UInt32) Marshal.SizeOf<ExtendedLimits>()))
                throw new Win32Exception();
            var startup = new StartupInfo {
                Size = (UInt32) Marshal.SizeOf<StartupInfo>(),
                Desktop = "WinSta0\\" + name,
            };
            var command = new StringBuilder(QuoteArgument(application));
            foreach (string argument in arguments)
                command.Append(' ').Append(QuoteArgument(argument));
            ProcessInfo process;
            if (!CreateProcess(application, command, IntPtr.Zero, IntPtr.Zero, false,
                CreateNoWindow | CreateSuspended, IntPtr.Zero, directory, ref startup, out process))
                throw new Win32Exception();
            try {
                // Attach before the PowerShell driver can start the player.
                if (!AssignProcessToJobObject(job, process.Process)) {
                    int error = Marshal.GetLastWin32Error();
                    TerminateProcess(process.Process, 124);
                    WaitForSingleObject(process.Process, 5000);
                    throw new Win32Exception(error);
                }
                if (ResumeThread(process.Thread) == UInt32.MaxValue)
                    throw new Win32Exception();

                UInt32 wait = WaitForSingleObject(process.Process, timeout);
                if (wait == WaitTimeout) {
                    TerminateJobObject(job, 124);
                    WaitForSingleObject(process.Process, 5000);
                    throw new TimeoutException("The isolated smoke driver did not exit in time.");
                }
                if (wait != 0)
                    throw new Win32Exception();
                UInt32 status;
                if (!GetExitCodeProcess(process.Process, out status))
                    throw new Win32Exception();
                return status;
            } finally {
                CloseHandle(process.Thread);
                CloseHandle(process.Process);
            }
        } finally {
            CloseHandle(job);
        }
    }

    private static string QuoteArgument(string argument)
    {
        // Win32 passes a command line. Escape quotes and the backslashes
        // adjacent to them so pwsh receives each original argument.
        var quoted = new StringBuilder("\"");
        int slashes = 0;
        foreach (char character in argument) {
            if (character == '\\') {
                slashes++;
                continue;
            }
            if (character == '"')
                quoted.Append('\\', slashes + 1);
            quoted.Append('\\', slashes).Append(character);
            slashes = 0;
        }
        return quoted.Append('\\', slashes * 2).Append('"').ToString();
    }
}
