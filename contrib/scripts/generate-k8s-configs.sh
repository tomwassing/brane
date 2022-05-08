# GENERATE KUBERNETES CONFIGS.sh
#   by Lut99
#
# Created:
#   03 May 2022, 14:05:02
# Last edited:
#   07 May 2022, 23:15:22
# Auto updated?
#   Yes
#
# Description:
#   Simple script that automatically generates a single Kubernetes file from
#   one of the docker-compose files.
#
#   Automatically downloads kompose and kustomize for this.
#


##### CONSTANTS #####
# Location & name of the downloaded Kompose executable
KOMPOSE="/tmp/kompose"
# Location & name of the downloaded yq (yaml parser) executable
YQ="/tmp/yq"
# # Location & name of the Kustomize tar file
# KUSTOMIZE_TAR="/tmp/kustomize.tar.gz"
# # Location & name of the extracted Kustomize executable
# KUSTOMIZE="/tmp/kustomize"
# Location of all temporary kubernetes resource files
BRANE_RESOURCES="/tmp/brane_k8s_resources"
# # Name of the Kustomization file (appended to BRANE_RESOURCES)
# KUSTOMIZE_FILE="kustomization.yml"

# The list of services we'll generate
SERVICES=(aux-scylla aux-registry aux-kafka aux-zookeeper brane-xenon aux-minio aux-redis once-format brane-api brane-clb brane-drv brane-job brane-log brane-plr)





##### HELPER FUNCTIONS #####
# Helper function that executes a command
exec_cmd() {
    # Construct a string from the input to show to user
    local cmd=""
    for arg in "$@"; do
        if [[ "$arg" =~ \  ]]; then
            cmd="$cmd \"$arg\""
        else
            cmd="$cmd $arg"
        fi
    done
    echo " >$cmd"

    # Run the recursive call with the error check
    "$@" || exit $?
}





##### CLI #####
# Read the input file with a slightly clever parser
input=""
output=""
storage_class_name=""
keep_temp=0
registry="127.0.0.1:50050"
cluster_domain="cluster.local"

state="start"
pos_i=0
allow_opts=1
errored=0
for arg in "$@"; do
    # Switch between states
    if [[ "$state" == "start" ]]; then
        # Switch between option or not
        if [[ "$allow_opts" -eq 1 && "$arg" =~ ^- ]]; then
            # Match the specific option
            if [[ "$arg" == "-k" || "$arg" == "--keep-temp" ]]; then
                # Mark that we keep the temporary files
                keep_temp=1

            elif [[ "$arg" == "-r" || "$arg" == "--registry" ]]; then
                # PArse the value next iteration
                state="registry"

            elif [[ "$arg" == "-c" || "$arg" == "--cluster-domain" ]]; then
                # PArse the value next iteration
                state="cluster-domain"

            elif [[ "$arg" == "-h" || "$arg" == "--help" ]]; then
                # Show the help string
                echo ""
                echo "Usage: $0 [opts] <docker compose file> <output dir>"
                echo ""
                echo "This script converts the given Docker Compose file into a Kubernetes resource file using Kompose and"
                echo "Kustomize."
                echo ""
                echo "It will generate two resource files: one registry file, which will setup a local Docker registry in"
                echo "the cluster, and one file for other Brane resources."
                echo ""
                echo "This is done because, in constrast to a local setup, the Docker registry will not just contain"
                echo "package images; it will also contain the other Brane images so they are available from within the"
                echo "entire cluster."
                echo ""
                echo "Positionals:"
                echo "  <docker compose file>  The file that will be converted to a Kubernetes resource file."
                echo "  <output dir>           The output directory where the Kubernetes files will be written to."
                echo "  <storage class name>   The name of the storage class to which to attach the POD persistent"
                echo "                         storage."
                echo ""
                echo "Options:"
                echo "  -k,--keep-temp         If given, keeps the temporary output files instead of removing them. The"
                echo "                         location of these temporary output files are provided as constants within"
                echo "                         the script itself."
                echo "  -r,--registry <address>"
                echo "                         The address (as \"hostname[:port]\") where the local image registry can be"
                echo "                         found of the Brane instance. Default: \"127.0.0.1:50050\""
                echo "  -c,--cluster-domain <domain>"
                echo "                         The name of the cluster, used for generating resolveable service DNS names."
                echo "                         Default: \"cluster.local\""
                echo "  -h,--help              Shows this help menu, then quits."
                echo "  --                     Any following values are interpreted as-is instead of as options."
                echo ""

                # Done, quit
                exit 0

            elif [[ "$arg" == "--" ]]; then
                # No longer allow options
                allow_opts=0

            else
                echo "Unknown option '$arg'"
                errored=1
            fi
        
        else
            # Match the positional index
            if [[ "$pos_i" -eq 0 ]]; then
                # It's the input file
                input=$(realpath "$arg")
            elif [[ "$pos_i" -eq 1 ]]; then
                # It's the output path
                output=$(realpath "$arg")
            elif [[ "$pos_i" -eq 2 ]]; then
                # It's the name of the storage class
                storage_class_name="$arg"
            else
                echo "Unknown positional '$arg' at index $pos_i"
                errored=1
            fi

            # Increment the index
            ((pos_i=pos_i+1))
        fi

    elif [[ "$state" == "registry" ]]; then
        # Switch between option or not
        if [[ "$allow_opts" -eq 1 && "$arg" =~ ^- ]]; then
            echo "Missing value for '--registry'"
            errored=1

        else
            # Simply set it
            registry="$arg"

        fi

        # Move back to the main state
        state="start"

    elif [[ "$state" == "cluster-domain" ]]; then
        # Switch between option or not
        if [[ "$allow_opts" -eq 1 && "$arg" =~ ^- ]]; then
            echo "Missing value for '--cluster-domain'"
            errored=1

        else
            # Simply set it
            cluster_domain="$arg"

        fi

        # Move back to the main state
        state="start"

    else
        echo "ERROR: Unknown state '$state'"
        exit 1

    fi
done

# If we're not in a start state, we didn't exist cleanly
if [[ "$state" == "registry" ]]; then
    echo "Missing value for '--registry'"
    errored=1

elif [[ "$state" == "cluster-domain" ]]; then
    echo "Missing value for '--cluster-domain'"
    errored=1

elif [[ "$state" != "start" ]]; then
    echo "ERROR: Unknown state '$state'"
    exit 1
fi

# Check if mandatory variables are given
if [[ -z "$input" ]]; then
    echo "No input file given; nothing to do."
    errored=1

elif [[ -z "$output" ]]; then
    echo "No output directory given"
    errored=1

elif [[ -z "$storage_class_name" ]]; then
    echo "No storage class name given"
    errored=1

fi

# If an error occurred, go no further
if [[ "$errored" -ne 0 ]]; then
    exit 1
fi





##### MAIN #####
# Start by downloading Kompose to the tmp dir
if [[ -f "$KOMPOSE" ]]; then
    echo "'$KOMPOSE' already exists, not downloading"
else
    # TODO: Switch between darwin and linux
    exec_cmd curl -L https://github.com/kubernetes/kompose/releases/download/v1.26.1/kompose-linux-amd64 -o "$KOMPOSE"
    exec_cmd chmod +x "$KOMPOSE"
fi

# Next, download yq
if [[ -f "$YQ" ]]; then
    echo "'$YQ' already exists, not downloading"
else
    # TODO: Switch between darwin and linux
    exec_cmd curl -L https://github.com/mikefarah/yq/releases/download/v4.25.1/yq_linux_amd64 -o "$YQ"
    exec_cmd chmod +x "$YQ"
fi

# # Next, download Kustomization (the tar) and extract it
# if [[ -f "$KUSTOMIZE" ]]; then
#     echo "'$KUSTOMIZE' already exists, not downloading"
# else
#     exec_cmd curl -L https://github.com/kubernetes-sigs/kustomize/releases/download/kustomize%2Fv4.5.4/kustomize_v4.5.4_linux_amd64.tar.gz -o "$KUSTOMIZE_TAR"
#     exec_cmd tar -xvzf "$KUSTOMIZE_TAR" -C $(dirname "$KUSTOMIZE") kustomize
#     exec_cmd chmod +x "$KUSTOMIZE"
# fi

# Then, run Kompose on the files
exec_cmd rm -rf "$BRANE_RESOURCES"
exec_cmd mkdir -p "$BRANE_RESOURCES"
echo " > cd \"$BRANE_RESOURCES\""
cd "$BRANE_RESOURCES"
exec_cmd "$KOMPOSE" --file "$input" convert

# Group all the different services together in one file
echo "Grouping services..."
exec_cmd mkdir -p "$output"
for svc in ${SERVICES[@]} "<network>"; do
    outfile="$output/$svc.yaml"

    # Do different service types
    if [[ "$svc" == "<network>" ]]; then
        # The network has a separate file
        files=("$BRANE_RESOURCES/brane-networkpolicy.yaml")
        outfile="$output/brane-networkpolicy.yaml"

    elif [[ "$svc" == "aux-zookeeper" || "$svc" == "once-format" || "$svc" == "brane-job" || "$svc" == "brane-plr" ]]; then
        # A few services are only deployment (never accessed from the outside)
        files=("$BRANE_RESOURCES/$svc-deployment.yaml")

        # Add the brane-job's volume claim, tho
        if [[ "$svc" == "brane-job" ]]; then
            files+=("$BRANE_RESOURCES/brane-job-claim0-persistentvolumeclaim.yaml")
        fi

        # Replace the image in once-format and brane-job
        if [[ "$svc" == "once-format" ]]; then
            printf "    "
            exec_cmd "$YQ" -i ".spec.template.spec.containers[0].image = \"$registry/brane-format:latest\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
        elif [[ "$svc" == "brane-job" ]]; then
            printf "    "
            exec_cmd "$YQ" -i ".spec.template.spec.containers[0].image = \"$registry/$svc:latest\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
        fi

        # Depending on the service, replace certain environment variables with resolveable DNS names
        if [[ "$svc" == "brane-job" ]]; then
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"BROKERS\")).value = \"aux-kafka.brane-control.svc.$cluster_domain:29092\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"XENON\")).value = \"brane-xenon.brane-control.svc.$cluster_domain:50054\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
            printf "    "
            exec_cmd "$YQ" -i ".spec.storageClassName = \"$storage_class_name\"" "$BRANE_RESOURCES/brane-job-claim0-persistentvolumeclaim.yaml"
        elif [[ "$svc" == "brane-plr" ]]; then
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"BROKERS\")).value = \"aux-kafka.brane-control.svc.$cluster_domain:29092\"" "$BRANE_RESOURCES/$svc-deployment.yaml"

        fi

    else
        # The rest has a deployment and a service per piece
        files=("$BRANE_RESOURCES/$svc-service.yaml" "$BRANE_RESOURCES/$svc-deployment.yaml")

        # Some of these services also have a claim
        if [[ "$svc" == "aux-minio" ]]; then
            files+=("$BRANE_RESOURCES/data-persistentvolumeclaim.yaml")
        fi

        # Replace the image for all of the brane services
        if [[ "$svc" =~ ^brane- ]]; then
            printf "    "
            exec_cmd "$YQ" -i ".spec.template.spec.containers[0].image = \"$registry/$svc:latest\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
        fi

        # Depending on the service, replace certain environment variables with resolveable DNS names
        if [[ "$svc" == "aux-kafka" ]]; then
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"KAFKA_ADVERTISED_LISTENERS\")).value = \"DOCKER://aux-kafka.brane-control.svc.$cluster_domain:29092,HOST://localhost:9092\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"KAFKA_LISTENERS\")).value = \"DOCKER://aux-kafka.brane-control.svc.$cluster_domain:29092,HOST://aux-kafka.brane-control.svc.$cluster_domain:9092\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"KAFKA_ZOOKEEPER_CONNECT\")).value = \"aux-zookeeper.brane-control.svc.$cluster_domain:2181\"" "$BRANE_RESOURCES/$svc-deployment.yaml"

        elif [[ "$svc" == "aux-minio" ]]; then
            printf "    "
            exec_cmd "$YQ" -i ".spec.storageClassName = \"$storage_class_name\"" "$BRANE_RESOURCES/data-persistentvolumeclaim.yaml"

        elif [[ "$svc" == "brane-api" ]]; then
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"REGISTRY\")).value = \"aux-registry.brane-control.svc.$cluster_domain:5000\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"SCYLLA\")).value = \"aux-scylla.brane-control.svc.$cluster_domain:9042\"" "$BRANE_RESOURCES/$svc-deployment.yaml"

        elif [[ "$svc" == "brane-clb" ]]; then
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"BROKERS\")).value = \"aux-kafka.brane-control.svc.$cluster_domain:29092\"" "$BRANE_RESOURCES/$svc-deployment.yaml"

        elif [[ "$svc" == "brane-drv" ]]; then
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"BROKERS\")).value = \"aux-kafka.brane-control.svc.$cluster_domain:29092\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"GRAPHQL_URL\")).value = \"http://brane-api.brane-control.svc.$cluster_domain:50051/graphql\"" "$BRANE_RESOURCES/$svc-deployment.yaml"

        elif [[ "$svc" == "brane-log" ]]; then
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"BROKERS\")).value = \"aux-kafka.brane-control.svc.$cluster_domain:29092\"" "$BRANE_RESOURCES/$svc-deployment.yaml"
            printf "    "
            exec_cmd "$YQ" -i "(.spec.template.spec.containers[0].env[] | select(.name == \"SCYLLA\")).value = \"aux-scylla.brane-control.svc.$cluster_domain:9042\"" "$BRANE_RESOURCES/$svc-deployment.yaml"

        fi
    fi

    # Merge the file by appending them
    echo " > rm -f \"$outfile\" && touch \"$outfile\""
    (rm -f "$outfile" && touch "$outfile") || exit $?
    for file in ${files[@]}; do
        if [[ ! -z "$(cat "$outfile")" ]]; then echo "---" >> "$outfile"; fi
        echo "     > cat \"$file\" >> \"$outfile\""
        (cat "$file" >> "$outfile") || exit $?
    done
done

# # Generates  Kustomize file in that directory that generates the registry file
# echo "Generating '$BRANE_RESOURCES/$KUSTOMIZE_FILE' file for Docker image / package registry..."
# cat <<EOT > "$BRANE_RESOURCES/$KUSTOMIZE_FILE"
# apiVersion: kustomize.config.k8s.io/v1beta1
# kind: Kustomization

# resources:
# - "$BRANE_RESOURCES/aux-registry-deployment.yaml"
# - "$BRANE_RESOURCES/aux-registry-service.yaml"
# EOT

# # Run Kustomize for this file
# echo " > $KUSTOMIZE build . > \"$output_reg\""
# bash -c "\"$KUSTOMIZE\" build . > \"$output_reg\"" || exit $?

# # Now generate a Kustomize file in that directory for other resources
# echo "Generating '$BRANE_RESOURCES/$KUSTOMIZE_FILE' file for Brane resources..."
# cat <<EOT > "$BRANE_RESOURCES/$KUSTOMIZE_FILE"
# apiVersion: kustomize.config.k8s.io/v1beta1
# kind: Kustomization

# resources:
# EOT
# for resource in "$BRANE_RESOURCES"/*; do
#     # Skip if the kustomization file
#     if [[ "$resource" == "$BRANE_RESOURCES/$KUSTOMIZE_FILE" ]]; then continue; fi
#     # Skip if the registry file(s)
#     if [[ "$resource" == "$BRANE_RESOURCES/aux-registry-deployment.yaml" ]]; then continue; fi
#     if [[ "$resource" == "$BRANE_RESOURCES/aux-registry-service.yaml" ]]; then continue; fi
    
#     # Otherwise, append line
#     echo "- \"$resource\"" >> "$BRANE_RESOURCES/$KUSTOMIZE_FILE"
# done

# # Run Kustomize
# echo " > $KUSTOMIZE build . > \"$output\""
# bash -c "\"$KUSTOMIZE\" build . > \"$output\"" || exit $?

# Cleanup
if [[ "$keep_temp" -ne 1 ]]; then
    echo "Cleaning up..."
    exec_cmd rm -rf "$BRANE_RESOURCES"
    # exec_cmd rm -f "$KUSTOMIZE_TAR"
fi

# Done
echo ""
echo "Done."
echo ""
