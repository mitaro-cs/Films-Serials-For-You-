// MSI Mystic Light SDK bridge. Only brightness changes; original values survive an app restart.
using System;
using System.IO;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Web.Script.Serialization;

public sealed class LedState {
    public string Device { get; set; }
    public uint Index { get; set; }
    public uint Brightness { get; set; }
}
public static class NovaNightLight {
    const string Dll = "MysticLight_SDK_x64.dll";
    [DllImport(Dll, CallingConvention=CallingConvention.Cdecl)] static extern int MLAPI_Initialize();
    [DllImport(Dll, CallingConvention=CallingConvention.Cdecl)] static extern int MLAPI_Release();
    [DllImport(Dll, CallingConvention=CallingConvention.Cdecl)] static extern int MLAPI_GetDeviceInfo(
      [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType=VarEnum.VT_BSTR)] out string[] devices,
      [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType=VarEnum.VT_BSTR)] out string[] counts);
    [DllImport(Dll, CallingConvention=CallingConvention.Cdecl)] static extern int MLAPI_GetLedBright([MarshalAs(UnmanagedType.BStr)] string device,uint index,out uint value);
    [DllImport(Dll, CallingConvention=CallingConvention.Cdecl)] static extern int MLAPI_SetLedBright([MarshalAs(UnmanagedType.BStr)] string device,uint index,uint value);
    static readonly JavaScriptSerializer Json=new JavaScriptSerializer();
    static void Save(string path,List<LedState> rows) {
        Directory.CreateDirectory(Path.GetDirectoryName(path));
        string temp=path+".tmp";
        File.WriteAllText(temp,Json.Serialize(rows));
        if(File.Exists(path)) File.Replace(temp,path,null); else File.Move(temp,path);
    }
    static void Reply(object value) { Console.WriteLine(Json.Serialize(value)); }
    public static int Main(string[] args) {
        if(args.Length!=2 || (args[0]!="probe"&&args[0]!="off"&&args[0]!="restore")) return 2;
        bool initialized=false;
        try {
            int code=MLAPI_Initialize();
            if(code!=0) {Reply(new {ok=false,error="MSI Mystic Light недоступен",code=code}); return 1;}
            initialized=true;
            var rows=new List<LedState>();int skipped=0;
            bool saved=File.Exists(args[1]);
            if(saved) rows=Json.Deserialize<List<LedState>>(File.ReadAllText(args[1]));
            else if(args[0]!="restore") {
                string[] devices,counts;
                code=MLAPI_GetDeviceInfo(out devices,out counts);
                if(code!=0 || devices==null || counts==null || devices.Length!=counts.Length) {Reply(new {ok=false,error="Не удалось прочитать устройства Mystic Light",code=code,devices=devices==null?-1:devices.Length,counts=counts==null?-1:counts.Length});return 1;}
                for(int d=0;d<devices.Length;d++) {
                    uint count;
                    if(!uint.TryParse(counts[d],out count)||count>4096) {skipped++;continue;}
                    for(uint index=0;index<count;index++) {
                        uint brightness;
                        if(MLAPI_GetLedBright(devices[d],index,out brightness)==0) rows.Add(new LedState {Device=devices[d],Index=index,Brightness=brightness});
                        else skipped++;
                    }
                }
            }
            if(args[0]=="probe") {Reply(new {ok=true,supported=rows.Count,skipped=skipped,saved=saved});return 0;}
            if(args[0]=="off" && !saved) Save(args[1],rows); // Commit restore data BEFORE the first change.
            var failed=new List<LedState>();int changed=0;
            foreach(var row in rows) {
                if(MLAPI_SetLedBright(row.Device,row.Index,args[0]=="off"?0:row.Brightness)==0) changed++;
                else failed.Add(row);
            }
            if(args[0]=="restore") {
                if(failed.Count==0) {if(File.Exists(args[1]))File.Delete(args[1]);}
                else Save(args[1],failed);
            }
            Reply(new {ok=failed.Count==0,changed=changed,failed=failed.Count,skipped=skipped});
            return failed.Count==0?0:1;
        } catch(Exception error) {Reply(new {ok=false,error=error.Message});return 1;}
        finally {if(initialized)MLAPI_Release();}
    }
}
