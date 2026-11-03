-- yavs-test: std=2008
-- expect: E1700 11:10 the instantiated name is the label itself
-- LRM 2008 11.1, 12.4: the label is directly visible and hides the component a use clause makes potentially visible (#1084 guard)
package label_comp_pkg is
  component fifo is end component;
end package;
use work.label_comp_pkg.all;
entity label_comp is end entity;
architecture a of label_comp is
begin
  fifo : fifo;
end architecture;
