import com.google.gson.*;
import com.google.gson.annotations.*;
import com.google.gson.reflect.TypeToken;
import com.google.gson.stream.*;
import java.io.*;
import java.lang.reflect.Type;
import java.util.*;

public class GsonCapabilities {
    static int passed = 0;
    static int failed = 0;

    static void check(String name, boolean ok) {
        if (ok) {
            passed++;
        } else {
            failed++;
            System.out.println("FAIL: " + name);
        }
    }

    static void json(String name, String expected, String actual) {
        try {
            JsonElement e = JsonParser.parseString(expected);
            JsonElement a = JsonParser.parseString(actual);
            check(name + " expected=" + expected + " actual=" + actual, e.equals(a));
        } catch (Throwable t) {
            check(name + " parse error " + t + " actual=" + actual, false);
        }
    }

    static void eq(String name, Object expected, Object actual) {
        check(name + " expected=" + expected + " actual=" + actual,
                Objects.equals(expected, actual));
    }

    static class Basic {
        int i = 7;
        String s = "x";
        boolean b = true;
        double d = 1.5;
        transient int ignored = 9;
        static int STATIC = 11;
    }

    static class Naming {
        int intField = 1;
        String stringField = "x";
    }

    static class NullHolder {
        String s = null;
        int i = 1;
    }

    static class Renamed {
        @SerializedName(value = "main", alternate = {"alt1", "alt2"})
        int x = 4;
    }

    static class Exposed {
        @Expose
        int a = 1;
        @Expose(serialize = false)
        int b = 2;
        int c = 3;
    }

    static class Versioned {
        @Since(1.0)
        int since = 1;
        @Until(2.0)
        int until = 2;
        int always = 3;
    }

    enum Color { RED, GREEN }

    static class DateHolder {
        Date date;
    }

    static class LongHolder {
        long value = 123L;
    }

    static class Custom {
        int value;
    }

    static class Point {
        int x;
        int y;
        Point() {}
        Point(int x, int y) { this.x = x; this.y = y; }
    }

    static class PointAdapter extends TypeAdapter<Point> {
        @Override
        public void write(JsonWriter out, Point p) throws IOException {
            if (p == null) { out.nullValue(); return; }
            out.value(p.x + "," + p.y);
        }
        @Override
        public Point read(JsonReader in) throws IOException {
            String s = in.nextString();
            String[] parts = s.split(",");
            return new Point(Integer.parseInt(parts[0]), Integer.parseInt(parts[1]));
        }
    }

    static class PointHolder {
        @JsonAdapter(PointAdapter.class)
        Point p = new Point(1, 2);
    }

    static class FactoryType {
        String v;
        FactoryType(String v) { this.v = v; }
    }

    static class FactoryTypeAdapter extends TypeAdapter<FactoryType> {
        @Override
        public void write(JsonWriter out, FactoryType value) throws IOException {
            out.beginObject();
            out.name("factory").value(value.v);
            out.endObject();
        }
        @Override
        public FactoryType read(JsonReader in) throws IOException {
            in.beginObject();
            in.nextName();
            String v = in.nextString();
            in.endObject();
            return new FactoryType(v);
        }
    }

    static class NoDefault {
        final int v;
        NoDefault(int v) { this.v = v; }
    }

    record Person(String name, int age) {}

    public static void main(String[] args) throws Exception {
        TimeZone.setDefault(TimeZone.getTimeZone("UTC"));
        Gson gson = new Gson();

        // basic object
        Basic basic = new Basic();
        json("basic serialize", "{\"i\":7,\"s\":\"x\",\"b\":true,\"d\":1.5}", gson.toJson(basic));
        Basic basic2 = gson.fromJson("{\"i\":9,\"s\":\"y\",\"b\":false,\"d\":2.5}", Basic.class);
        eq("basic deserialize i", 9, basic2.i);
        eq("basic deserialize s", "y", basic2.s);
        check("basic deserialize ignored transient", basic2.ignored == 9);

        // collections and arrays
        List<Basic> list = new ArrayList<>();
        list.add(basic);
        json("list serialize", "[{\"i\":7,\"s\":\"x\",\"b\":true,\"d\":1.5}]", gson.toJson(list));
        Type listType = new TypeToken<List<Basic>>() {}.getType();
        List<Basic> list2 = gson.fromJson("[{\"i\":1,\"s\":\"a\",\"b\":false,\"d\":0.5}]", listType);
        eq("list deserialize size", 1, list2.size());
        eq("list deserialize i", 1, list2.get(0).i);

        Map<String, List<Basic>> map = new LinkedHashMap<>();
        map.put("k", list);
        json("map serialize", "{\"k\":[{\"i\":7,\"s\":\"x\",\"b\":true,\"d\":1.5}]}", gson.toJson(map));
        Type mapType = new TypeToken<Map<String, List<Basic>>>() {}.getType();
        Map<String, List<Basic>> map2 = gson.fromJson("{\"k\":[{\"i\":2,\"s\":\"b\",\"b\":true,\"d\":1.0}]}", mapType);
        eq("map deserialize nested i", 2, map2.get("k").get(0).i);

        int[] ints = {1, 2, 3};
        json("array serialize", "[1,2,3]", gson.toJson(ints));
        int[] ints2 = gson.fromJson("[4,5]", int[].class);
        eq("array deserialize", 5, ints2[1]);

        Set<String> set = new LinkedHashSet<>(Arrays.asList("a", "b"));
        json("set serialize", "[\"a\",\"b\"]", gson.toJson(set));

        // enum
        json("enum serialize", "\"RED\"", gson.toJson(Color.RED));
        eq("enum deserialize", Color.GREEN, gson.fromJson("\"GREEN\"", Color.class));

        // naming policy
        Gson naming = new GsonBuilder().setFieldNamingPolicy(FieldNamingPolicy.LOWER_CASE_WITH_UNDERSCORES).create();
        json("naming policy", "{\"int_field\":1,\"string_field\":\"x\"}", naming.toJson(new Naming()));

        // custom field naming strategy
        Gson customNaming = new GsonBuilder().setFieldNamingStrategy(f -> "x_" + f.getName()).create();
        json("custom naming strategy", "{\"x_intField\":1,\"x_stringField\":\"x\"}", customNaming.toJson(new Naming()));

        // null / serializeNulls
        json("omit nulls", "{\"i\":1}", gson.toJson(new NullHolder()));
        Gson nulls = new GsonBuilder().serializeNulls().create();
        json("serialize nulls", "{\"s\":null,\"i\":1}", nulls.toJson(new NullHolder()));

        // pretty printing and html escaping
        String pretty = new GsonBuilder().setPrettyPrinting().create().toJson(new Basic());
        check("pretty printing newline", pretty.contains("\n"));
        json("html escaping", "\"\\u003cb\\u003e\"", gson.toJson("<b>"));
        json("disable html escaping", "\"<b>\"", new GsonBuilder().disableHtmlEscaping().create().toJson("<b>"));

        // SerializedName alternate
        json("SerializedName", "{\"main\":4}", gson.toJson(new Renamed()));
        Renamed renamed = gson.fromJson("{\"alt2\":5}", Renamed.class);
        eq("SerializedName alternate read", 5, renamed.x);

        // Expose
        Gson exposed = new GsonBuilder().excludeFieldsWithoutExposeAnnotation().create();
        json("Expose serialize", "{\"a\":1}", exposed.toJson(new Exposed()));
        Exposed ex = exposed.fromJson("{\"a\":9,\"b\":8,\"c\":7}", Exposed.class);
        eq("Expose a", 9, ex.a);
        eq("Expose b deserialize", 8, ex.b);
        eq("Expose c hidden keeps initializer", 3, ex.c);

        // Since / Until
        Gson v15 = new GsonBuilder().setVersion(1.5).create();
        json("Since/Until 1.5", "{\"since\":1,\"until\":2,\"always\":3}", v15.toJson(new Versioned()));
        Gson v20 = new GsonBuilder().setVersion(2.0).create();
        json("Since/Until 2.0", "{\"since\":1,\"always\":3}", v20.toJson(new Versioned()));
        Gson v05 = new GsonBuilder().setVersion(0.5).create();
        json("Since/Until 0.5", "{\"until\":2,\"always\":3}", v05.toJson(new Versioned()));

        // Date
        DateHolder dh = new DateHolder();
        dh.date = new Date(0);
        Gson dateGson = new GsonBuilder().setDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'").create();
        json("date format", "{\"date\":\"1970-01-01T00:00:00Z\"}", dateGson.toJson(dh));
        DateHolder dh2 = dateGson.fromJson("{\"date\":\"1970-01-01T00:00:00Z\"}", DateHolder.class);
        eq("date parse", 0L, dh2.date.getTime());

        // long policy
        Gson longGson = new GsonBuilder().setLongSerializationPolicy(LongSerializationPolicy.STRING).create();
        json("long string policy", "{\"value\":\"123\"}", longGson.toJson(new LongHolder()));

        // special floating point
        Gson special = new GsonBuilder().serializeSpecialFloatingPointValues().create();
        check("NaN", "NaN".equals(special.toJson(Double.NaN)));
        check("Infinity", "Infinity".equals(special.toJson(Double.POSITIVE_INFINITY)));

        // custom serializer / deserializer
        Gson custom = new GsonBuilder()
                .registerTypeAdapter(Custom.class, (JsonSerializer<Custom>) (src, t, c) -> new JsonPrimitive("custom:" + src.value))
                .registerTypeAdapter(Custom.class, (JsonDeserializer<Custom>) (json, t, c) -> {
                    Custom out = new Custom();
                    out.value = Integer.parseInt(json.getAsString().substring("custom:".length()));
                    return out;
                })
                .create();
        Custom c = new Custom();
        c.value = 7;
        json("custom serializer", "\"custom:7\"", custom.toJson(c));
        eq("custom deserializer", 9, custom.fromJson("\"custom:9\"", Custom.class).value);

        // @JsonAdapter
        json("@JsonAdapter", "{\"p\":\"1,2\"}", gson.toJson(new PointHolder()));
        PointHolder ph = gson.fromJson("{\"p\":\"3,4\"}", PointHolder.class);
        eq("@JsonAdapter x", 3, ph.p.x);
        eq("@JsonAdapter y", 4, ph.p.y);

        // TypeAdapterFactory
        Gson factory = new GsonBuilder().registerTypeAdapterFactory(new TypeAdapterFactory() {
            @Override
            public <T> TypeAdapter<T> create(Gson g, TypeToken<T> type) {
                if (type.getRawType() != FactoryType.class) return null;
                @SuppressWarnings("unchecked")
                TypeAdapter<T> adapter = (TypeAdapter<T>) new FactoryTypeAdapter();
                return adapter;
            }
        }).create();
        json("TypeAdapterFactory", "{\"factory\":\"ok\"}", factory.toJson(new FactoryType("ok")));
        eq("TypeAdapterFactory read", "ok", factory.fromJson("{\"factory\":\"ok\"}", FactoryType.class).v);

        // InstanceCreator
        Gson instance = new GsonBuilder()
                .registerTypeAdapter(NoDefault.class, (InstanceCreator<NoDefault>) t -> new NoDefault(42))
                .create();
        NoDefault nd = instance.fromJson("{\"v\":7}", NoDefault.class);
        eq("InstanceCreator field set", 7, nd.v);

        // JsonElement tree
        JsonObject obj = new JsonObject();
        obj.addProperty("name", "n");
        JsonArray arr = new JsonArray();
        arr.add(1);
        arr.add(2);
        obj.add("nums", arr);
        json("JsonElement tree", "{\"name\":\"n\",\"nums\":[1,2]}", gson.toJson(obj));
        JsonElement parsed = JsonParser.parseString("{\"name\":\"m\",\"nums\":[3]}");
        eq("JsonElement get", "m", parsed.getAsJsonObject().get("name").getAsString());

        JsonObject basicTree = new JsonObject();
        basicTree.addProperty("s", "n");
        Basic fromTree = gson.fromJson(basicTree, Basic.class);
        eq("from JsonElement", "n", fromTree.s);

        Basic toTree = gson.fromJson(gson.toJsonTree(basic), Basic.class);
        eq("toJsonTree/fromJson", 7, toTree.i);

        // streaming JsonWriter / JsonReader
        StringWriter sw = new StringWriter();
        JsonWriter jw = new JsonWriter(sw);
        jw.beginObject();
        jw.name("a").value(1);
        jw.name("b").value("x");
        jw.endObject();
        jw.close();
        json("JsonWriter", "{\"a\":1,\"b\":\"x\"}", sw.toString());

        JsonReader jr = new JsonReader(new StringReader("{\"a\":1,\"b\":\"x\"}"));
        jr.beginObject();
        eq("JsonReader name a", "a", jr.nextName());
        eq("JsonReader value a", 1, jr.nextInt());
        eq("JsonReader name b", "b", jr.nextName());
        eq("JsonReader value b", "x", jr.nextString());
        jr.endObject();
        jr.close();

        // record
        json("record serialize", "{\"name\":\"p\",\"age\":3}", gson.toJson(new Person("p", 3)));
        Person p = gson.fromJson("{\"name\":\"q\",\"age\":4}", Person.class);
        eq("record name", "q", p.name());
        eq("record age", 4, p.age());

        // reflection access filter (user class still allowed)
        Gson filtered = new GsonBuilder()
                .addReflectionAccessFilter(ReflectionAccessFilter.BLOCK_ALL_PLATFORM)
                .create();
        json("reflection filter user class", "{\"i\":7,\"s\":\"x\",\"b\":true,\"d\":1.5}", filtered.toJson(basic));

        System.out.println("PASSED=" + passed + " FAILED=" + failed);
        if (failed != 0) {
            System.exit(1);
        }
    }
}
