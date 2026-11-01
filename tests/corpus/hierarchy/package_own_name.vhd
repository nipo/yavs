-- difftest: top=package_own_name std=2008
-- 'instance_name and 'path_name of a library package named by itself, in its constants, its body
-- and a subprogram of it (LRM 16.2.5, #1007).
package package_own_name_p is
  constant iname : string := package_own_name_p'instance_name;
  constant pname : string := package_own_name_p'path_name;
  constant cc : integer := 1;
  constant item : string := cc'instance_name;
  procedure proc;
  function fin return string;
  -- Below the library, whose name the harness of each simulator chooses.
  function tail (s : string) return string;
end package;
package body package_own_name_p is
  constant bn : string := package_own_name_p'instance_name;
  function tail (s : string) return string is
  begin
    for i in s'left + 1 to s'right loop
      if s(i) = ':' then
        return s(i to s'right);
      end if;
    end loop;
    return s;
  end;
  procedure proc is
  begin
    report tail(iname);
    report tail(pname);
    report tail(package_own_name_p'instance_name);
    report tail(package_own_name_p'path_name);
    report tail(bn);
    report tail(item);
  end;
  function fin return string is begin return package_own_name_p'instance_name; end;
end package body;
use work.package_own_name_p.all;
entity package_own_name is end;
architecture t of package_own_name is
begin
  process begin
    proc;
    report tail(fin);
    wait;
  end process;
end;
