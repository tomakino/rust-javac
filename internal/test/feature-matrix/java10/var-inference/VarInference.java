import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class VarInference {
    public static void main(String[] args) {
        var list = new ArrayList<String>();
        list.add("x");
        for (var element : list) {
            System.out.println(element);
        }
        var map = new HashMap<String, List<Integer>>();
        map.put("k", List.of(1, 2));
        for (Map.Entry<String, List<Integer>> entry : map.entrySet()) {
            System.out.println(entry.getKey() + entry.getValue().size());
        }
    }
}
