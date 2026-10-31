import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.util.ArrayList;
import java.util.List;

@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE_USE)
@interface A { }

public class NewAnnoHell {
    static class Plain { Plain() { } }

    private List<@A String> list = new @A ArrayList<@A String>();
    private Plain plain = new @A Plain();
    private Object anon = new @A Object() { };

    @SuppressWarnings("unchecked")
    private List<@A ?> wild = new ArrayList<@A String>();

    public static void main(String[] args) {
        NewAnnoHell n = new NewAnnoHell();
        n.anon = new @A Object() { @Override public String toString() { return "x"; } };
        Object o = n.list;
        System.out.println(o + "" + n.plain + n.anon + n.wild);
    }
}
