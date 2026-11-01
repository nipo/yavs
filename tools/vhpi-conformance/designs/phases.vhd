-- Tool phases and phase callbacks (plugins/phases.c): a foreign function (its elaboration
-- function runs in the elaboration phase) and a little activity.
entity phases is end entity;

architecture sim of phases is
  function f return integer;
  attribute foreign of f : function is "VHPI phases f";
  function f return integer is begin return 0; end function;
  signal s : integer := -1;
begin
  process
  begin
    wait for 1 ns;
    s <= f;
    wait for 1 ns;
    wait;
  end process;
end architecture;
